package chart

import (
	"database/sql"
	"encoding/json"
	"log"
	"net/http"
	"strconv"
	"time"

	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/candles"
)

type Handler struct {
	DB            *sql.DB
	EncryptionKey []byte
	DhanBaseURL   string
}

type Candle struct {
	Timestamp int64   `json:"timestamp"`
	Open      float64 `json:"open"`
	High      float64 `json:"high"`
	Low       float64 `json:"low"`
	Close     float64 `json:"close"`
	Volume    int64   `json:"volume"`
}

func (h *Handler) Candles(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	securityID := r.URL.Query().Get("security_id")
	exchangeSegment := r.URL.Query().Get("exchange_segment")
	interval := r.URL.Query().Get("interval")
	from := r.URL.Query().Get("from")
	to := r.URL.Query().Get("to")

	if userID == "" || securityID == "" || exchangeSegment == "" || interval == "" || from == "" || to == "" {
		http.Error(w, "missing required params", http.StatusBadRequest)
		return
	}

	if _, err := strconv.ParseInt(securityID, 10, 64); err != nil {
		http.Error(w, "invalid security_id", http.StatusBadRequest)
		return
	}

	clientID, accessToken, err := broker.GetToken(h.DB, h.EncryptionKey, userID)
	if err == sql.ErrNoRows {
		http.Error(w, "broker not connected", http.StatusUnauthorized)
		return
	}
	if err != nil {
		http.Error(w, "failed to get broker token", http.StatusInternalServerError)
		return
	}

	today := time.Now().Format("2006-01-02")

	// cold cache: fetch full range if no data exists
	var count int
	h.DB.QueryRowContext(r.Context(), `
		select count(*) from candles
		where security_id=$1 and exchange_segment=$2 and interval=$3
		and timestamp::date between $4::text::date and $5::text::date`,
		securityID, exchangeSegment, interval, from, to,
	).Scan(&count)

	// Fetch any missing data before the earliest stored candle in the requested range.
	var minDate string
	h.DB.QueryRowContext(r.Context(), `
		select min(timestamp::date)::text from candles
		where security_id=$1 and exchange_segment=$2 and interval=$3`,
		securityID, exchangeSegment, interval,
	).Scan(&minDate)

	if count == 0 || (minDate != "" && from < minDate) {
		fetchFrom := from
		fetchTo := to
		if count > 0 && minDate != "" {
			fetchTo = minDate
		}
		log.Printf("chart: fetching candles for %s %s %s %s–%s", securityID, exchangeSegment, interval, fetchFrom, fetchTo)
		if err := candles.FetchAndStore(h.DB, h.DhanBaseURL, clientID, accessToken, securityID, exchangeSegment, interval, fetchFrom, fetchTo); err != nil {
			log.Printf("chart: fetch error: %v", err)
			http.Error(w, "failed to fetch candles: "+err.Error(), http.StatusBadGateway)
			return
		}
	}

	if to >= today {
		// Refresh from the last stored candle date so gaps from multi-day absences are filled.
		var maxDate string
		h.DB.QueryRowContext(r.Context(), `
			select max(timestamp::date)::text from candles
			where security_id=$1 and exchange_segment=$2 and interval=$3`,
			securityID, exchangeSegment, interval,
		).Scan(&maxDate)
		if maxDate == "" {
			maxDate = today
		}
		log.Printf("chart: refreshing candles for %s %s %s from %s", securityID, exchangeSegment, interval, maxDate)
		if err := candles.FetchAndStore(h.DB, h.DhanBaseURL, clientID, accessToken, securityID, exchangeSegment, interval, maxDate, today, true); err != nil {
			log.Printf("chart: refresh error: %v", err)
			// non-fatal: serve what we have in DB
		}
	}

	rows, err := h.DB.QueryContext(r.Context(),
		`select extract(epoch from timestamp)::bigint, open, high, low, close, volume
		 from candles
		 where security_id=$1 and exchange_segment=$2 and interval=$3
		 and timestamp::date between $4::text::date and $5::text::date
		 order by timestamp`,
		securityID, exchangeSegment, interval, from, to,
	)
	if err != nil {
		http.Error(w, "query error", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	result := []Candle{}
	for rows.Next() {
		var c Candle
		if err := rows.Scan(&c.Timestamp, &c.Open, &c.High, &c.Low, &c.Close, &c.Volume); err != nil {
			http.Error(w, "scan error", http.StatusInternalServerError)
			return
		}
		result = append(result, c)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(result)
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /chart/candles", h.Candles)
}
