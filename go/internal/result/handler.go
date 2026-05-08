package result

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"

	"github.com/minio/minio-go/v7"
	"github.com/minio/minio-go/v7/pkg/credentials"
)

type Handler struct {
	DB     *sql.DB
	bucket string
	s3     *minio.Client
}

func NewHandler(db *sql.DB) (*Handler, error) {
	endpoint := os.Getenv("MINIO_ENDPOINT")
	user := os.Getenv("MINIO_ROOT_USER")
	pass := os.Getenv("MINIO_ROOT_PASSWORD")
	bucket := os.Getenv("MINIO_BUCKET")
	if bucket == "" {
		bucket = "dhan"
	}
	client, err := minio.New(endpoint, &minio.Options{
		Creds:  credentials.NewStaticV4(user, pass, ""),
		Secure: false,
	})
	if err != nil {
		return nil, fmt.Errorf("minio client: %w", err)
	}
	return &Handler{DB: db, bucket: bucket, s3: client}, nil
}

func (h *Handler) RunResult(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	runID := r.URL.Query().Get("run_id")
	if userID == "" || runID == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}

	var resultKey sql.NullString
	err := h.DB.QueryRowContext(r.Context(),
		`select r.result_key
		 from backtest_runs r
		 join strategies s on s.id = r.strategy_id
		 where r.id = $1 and s.user_id = $2`,
		runID, userID,
	).Scan(&resultKey)
	if err == sql.ErrNoRows {
		http.Error(w, "not found", http.StatusNotFound)
		return
	}
	if err != nil {
		http.Error(w, "db error", http.StatusInternalServerError)
		return
	}
	if !resultKey.Valid {
		http.Error(w, "result not ready", http.StatusNotFound)
		return
	}

	sigKey := fmt.Sprintf("runs/%s/signals.json", runID)
	obj, err := h.s3.GetObject(context.Background(), h.bucket, sigKey, minio.GetObjectOptions{})
	if err != nil {
		http.Error(w, "signals not found", http.StatusNotFound)
		return
	}
	defer obj.Close()

	data, err := io.ReadAll(obj)
	if err != nil {
		http.Error(w, "read error", http.StatusInternalServerError)
		return
	}

	var raw json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		http.Error(w, "invalid signals data", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.Write(data)
}

func (h *Handler) DeleteRun(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	runID := r.URL.Query().Get("run_id")
	if userID == "" || runID == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}

	var resultKey sql.NullString
	err := h.DB.QueryRowContext(r.Context(),
		`select r.result_key
		 from backtest_runs r
		 join strategies s on s.id = r.strategy_id
		 where r.id = $1 and s.user_id = $2`,
		runID, userID,
	).Scan(&resultKey)
	if err == sql.ErrNoRows {
		http.Error(w, "not found", http.StatusNotFound)
		return
	}
	if err != nil {
		http.Error(w, "db error", http.StatusInternalServerError)
		return
	}

	for _, key := range []string{
		fmt.Sprintf("runs/%s/result.parquet", runID),
		fmt.Sprintf("runs/%s/signals.json", runID),
	} {
		h.s3.RemoveObject(context.Background(), h.bucket, key, minio.RemoveObjectOptions{})
	}

	w.WriteHeader(http.StatusNoContent)
}

func (h *Handler) StrategySource(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	strategyID := r.URL.Query().Get("strategy_id")
	if userID == "" || strategyID == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}

	var sourceKey sql.NullString
	err := h.DB.QueryRowContext(r.Context(),
		`select source_key from strategies where id = $1 and user_id = $2`,
		strategyID, userID,
	).Scan(&sourceKey)
	if err == sql.ErrNoRows {
		http.Error(w, "not found", http.StatusNotFound)
		return
	}
	if err != nil || !sourceKey.Valid {
		http.Error(w, "source not available", http.StatusNotFound)
		return
	}

	// Before first build, source_key holds the raw snippet directly
	if len(sourceKey.String) > 0 && sourceKey.String[0] != 's' {
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte(sourceKey.String))
		return
	}

	obj, err := h.s3.GetObject(context.Background(), h.bucket, sourceKey.String, minio.GetObjectOptions{})
	if err != nil {
		http.Error(w, "source not found", http.StatusNotFound)
		return
	}
	defer obj.Close()

	full, err := io.ReadAll(obj)
	if err != nil {
		http.Error(w, "read error", http.StatusInternalServerError)
		return
	}

	// Extract snippet from inside fn signal { ... }
	snippet := extractSnippet(string(full))
	w.Header().Set("Content-Type", "text/plain")
	w.Write([]byte(snippet))
}

// extractSnippet pulls the user code from the wrapped Rust source.
// The builder wraps it as: fn signal(prices: &[f64], i: usize) -> u8 {\n<snippet>\n}
func extractSnippet(src string) string {
	const marker = "fn signal(prices: &[f64], i: usize) -> u8 {"
	start := strings.Index(src, marker)
	if start < 0 {
		return src
	}
	start += len(marker)
	// find matching closing brace
	depth := 1
	for i := start; i < len(src); i++ {
		switch src[i] {
		case '{':
			depth++
		case '}':
			depth--
			if depth == 0 {
				snippet := strings.TrimSpace(src[start:i])
				return snippet
			}
		}
	}
	return strings.TrimSpace(src[start:])
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /chart/run-result", h.RunResult)
	mux.HandleFunc("GET /chart/strategy-source", h.StrategySource)
	mux.HandleFunc("DELETE /chart/run", h.DeleteRun)
}
