package broker

import (
	"database/sql"

	"github.com/madhav165/dhan-test/go/internal/crypto"
)

func GetToken(db *sql.DB, key []byte, userID string) (clientID, accessToken string, err error) {
	var encrypted string
	err = db.QueryRow(
		`select client_id, encrypted_token from broker_connections
		 where user_id = $1 and broker = 'dhan'
		 and token_date = (current_timestamp at time zone 'Asia/Kolkata')::date
		 and is_active = true`,
		userID,
	).Scan(&clientID, &encrypted)
	if err != nil {
		return
	}

	plain, err := crypto.Decrypt(key, encrypted)
	if err != nil {
		return
	}
	accessToken = string(plain)
	return
}
