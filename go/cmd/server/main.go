package main

import (
	"encoding/hex"
	"log"
	"net/http"
	"os"

	"github.com/joho/godotenv"
	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/db"
	"github.com/madhav165/dhan-test/go/internal/market"
)

func main() {
	godotenv.Load("../.env")

	database, err := db.Connect(os.Getenv("DATABASE_URL"))
	if err != nil {
		log.Fatalf("failed to connect to db: %v", err)
	}
	defer database.Close()

	keyHex := os.Getenv("ENCRYPTION_KEY")
	key, err := hex.DecodeString(keyHex)
	if err != nil || len(key) != 32 {
		log.Fatal("ENCRYPTION_KEY must be a 64-char hex string (32 bytes)")
	}

	h := &broker.Handler{
		DB:             database,
		EncryptionKey:  key,
		InternalSecret: os.Getenv("INTERNAL_SECRET"),
	}

	mh := market.NewHandler(database, key, os.Getenv("DHAN_BASE_URL"))

	mux := http.NewServeMux()
	mux.HandleFunc("POST /internal/broker-token", h.StoreToken)
	mh.RegisterRoutes(mux)

	port := os.Getenv("GO_PORT")
	if port == "" {
		port = "8080"
	}

	log.Printf("Go service listening on :%s", port)
	log.Fatal(http.ListenAndServe(":"+port, mux))
}
