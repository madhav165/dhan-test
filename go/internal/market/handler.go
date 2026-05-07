package market

import (
	"bytes"
	"database/sql"
	"io"
	"net/http"

	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/ratelimit"
	"golang.org/x/time/rate"
)

type Handler struct {
	DB            *sql.DB
	EncryptionKey []byte
	DhanBaseURL   string
	quoteRL       *ratelimit.Store // 1 req/s per user
	dataRL        *ratelimit.Store // 5 req/s per user
}

func NewHandler(db *sql.DB, key []byte, baseURL string) *Handler {
	return &Handler{
		DB:            db,
		EncryptionKey: key,
		DhanBaseURL:   baseURL,
		quoteRL:       ratelimit.NewStore(rate.Every(1), 1),
		dataRL:        ratelimit.NewStore(5, 5),
	}
}

func (h *Handler) proxyDhan(endpoint string, rl *ratelimit.Store) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		userID := r.Header.Get("X-User-ID")
		if userID == "" {
			http.Error(w, "Unauthorized", http.StatusUnauthorized)
			return
		}

		if err := rl.Get(userID).Wait(r.Context()); err != nil {
			http.Error(w, "Rate limit exceeded", http.StatusTooManyRequests)
			return
		}

		clientID, accessToken, err := broker.GetToken(h.DB, h.EncryptionKey, userID)
		if err == sql.ErrNoRows {
			http.Error(w, "Broker not connected", http.StatusUnauthorized)
			return
		}
		if err != nil {
			http.Error(w, "Failed to get token", http.StatusInternalServerError)
			return
		}

		body, err := io.ReadAll(r.Body)
		if err != nil {
			http.Error(w, "Bad request", http.StatusBadRequest)
			return
		}

		req, err := http.NewRequest(r.Method, h.DhanBaseURL+endpoint, bytes.NewReader(body))
		if err != nil {
			http.Error(w, "Internal error", http.StatusInternalServerError)
			return
		}
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("access-token", accessToken)
		req.Header.Set("client-id", clientID)

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			http.Error(w, "Dhan API error", http.StatusBadGateway)
			return
		}
		defer resp.Body.Close()

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(resp.StatusCode)
		io.Copy(w, resp.Body)
	}
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /market/ltp", h.proxyDhan("/marketfeed/ltp", h.quoteRL))
	mux.HandleFunc("POST /market/ohlc", h.proxyDhan("/marketfeed/ohlc", h.quoteRL))
	mux.HandleFunc("POST /market/quote", h.proxyDhan("/marketfeed/quote", h.quoteRL))
}
