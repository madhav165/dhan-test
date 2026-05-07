package broker

import (
	"database/sql"
	"encoding/json"
	"net/http"

	"github.com/madhav165/dhan-test/go/internal/crypto"
)

type Handler struct {
	DB            *sql.DB
	EncryptionKey []byte
	InternalSecret string
}

type storeTokenRequest struct {
	UserID    string `json:"user_id"`
	Broker    string `json:"broker"`
	ClientID  string `json:"client_id"`
	Token     string `json:"token"`
	TokenDate string `json:"token_date"`
}

func (h *Handler) StoreToken(w http.ResponseWriter, r *http.Request) {
	if r.Header.Get("X-Internal-Secret") != h.InternalSecret {
		http.Error(w, "Unauthorized", http.StatusUnauthorized)
		return
	}

	var req storeTokenRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Bad request", http.StatusBadRequest)
		return
	}

	encrypted, err := crypto.Encrypt(h.EncryptionKey, []byte(req.Token))
	if err != nil {
		http.Error(w, "Encryption failed", http.StatusInternalServerError)
		return
	}

	_, err = h.DB.Exec(
		`insert into broker_connections (user_id, broker, client_id, encrypted_token, token_date, is_active)
		 values ($1, $2, $3, $4, current_date, true)
		 on conflict (user_id, broker) do update
		 set client_id = excluded.client_id,
		     encrypted_token = excluded.encrypted_token,
		     token_date = current_date,
		     is_active = true`,
		req.UserID, req.Broker, req.ClientID, encrypted,
	)
	if err != nil {
		http.Error(w, "DB error", http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusNoContent)
}
