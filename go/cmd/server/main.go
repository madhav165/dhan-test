package main

import (
	"embed"
	"encoding/hex"
	"log"
	"net/http"
	"os"

	"github.com/golang-migrate/migrate/v4"
	"github.com/golang-migrate/migrate/v4/database/postgres"
	"github.com/golang-migrate/migrate/v4/source/iofs"
	"github.com/joho/godotenv"
	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/chart"
	"github.com/madhav165/dhan-test/go/internal/db"
	"github.com/madhav165/dhan-test/go/internal/instrument"
	"github.com/madhav165/dhan-test/go/internal/live"
	"github.com/madhav165/dhan-test/go/internal/market"
	"github.com/madhav165/dhan-test/go/internal/result"
	"github.com/madhav165/dhan-test/go/internal/run"
)

//go:embed migrations
var migrationsFS embed.FS

func main() {
	godotenv.Load("../.env")

	database, err := db.Connect(os.Getenv("DATABASE_URL"))
	if err != nil {
		log.Fatalf("failed to connect to db: %v", err)
	}
	defer database.Close()

	driver, err := postgres.WithInstance(database, &postgres.Config{})
	if err != nil {
		log.Fatalf("migrate driver: %v", err)
	}
	src, err := iofs.New(migrationsFS, "migrations")
	if err != nil {
		log.Fatalf("migrate source: %v", err)
	}
	m, err := migrate.NewWithInstance("iofs", src, "postgres", driver)
	if err != nil {
		log.Fatalf("migrate init: %v", err)
	}
	if err := m.Up(); err != nil && err != migrate.ErrNoChange {
		log.Fatalf("migrate up: %v", err)
	}
	log.Println("migrations up to date")

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

	go instrument.RunScheduler(database)

	runWorker := &run.Worker{DB: database, EncKey: key, DhanBaseURL: os.Getenv("DHAN_BASE_URL")}
	go runWorker.Start()

	ih := &instrument.Handler{DB: database}
	mh := market.NewHandler(database, key, os.Getenv("DHAN_BASE_URL"))
	ch := &chart.Handler{DB: database, EncryptionKey: key, DhanBaseURL: os.Getenv("DHAN_BASE_URL")}
	lh := live.NewHandler(database, key)
	rh, err := result.NewHandler(database)
	if err != nil {
		log.Fatalf("result handler: %v", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("POST /internal/broker-token", h.StoreToken)
	mh.RegisterRoutes(mux)
	ih.RegisterRoutes(mux)
	ch.RegisterRoutes(mux)
	lh.RegisterRoutes(mux)
	rh.RegisterRoutes(mux)

	port := os.Getenv("GO_PORT")
	if port == "" {
		port = "8080"
	}

	log.Printf("Go service listening on :%s", port)
	log.Fatal(http.ListenAndServe(":"+port, mux))
}
