package market

import (
	"bytes"
	"database/sql"
	"io"
	"net/http"

	"github.com/madhav165/dhan-test/go/internal/broker"
)

type Handler struct {
	DB            *sql.DB
	EncryptionKey []byte
	DhanBaseURL   string
}

func (h *Handler) proxyDhan(endpoint string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		userID := r.Header.Get("X-User-ID")
		if userID == "" {
			http.Error(w, "Unauthorized", http.StatusUnauthorized)
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
	mux.HandleFunc("POST /market/ltp", h.proxyDhan("/marketfeed/ltp"))
	mux.HandleFunc("POST /market/ohlc", h.proxyDhan("/marketfeed/ohlc"))
	mux.HandleFunc("POST /market/quote", h.proxyDhan("/marketfeed/quote"))
}
