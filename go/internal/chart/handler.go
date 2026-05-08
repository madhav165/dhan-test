package chart

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"strconv"
)

type Handler struct {
	DB *sql.DB
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
	securityID, err := strconv.ParseInt(r.URL.Query().Get("security_id"), 10, 64)
	if err != nil {
		http.Error(w, "invalid security_id", http.StatusBadRequest)
		return
	}
	exchangeSegment := r.URL.Query().Get("exchange_segment")
	interval := r.URL.Query().Get("interval")
	from := r.URL.Query().Get("from")
	to := r.URL.Query().Get("to")

	if exchangeSegment == "" || interval == "" || from == "" || to == "" {
		http.Error(w, "missing required params", http.StatusBadRequest)
		return
	}

	rows, err := h.DB.QueryContext(r.Context(),
		`select timestamp, open, high, low, close, volume
		 from candles
		 where security_id = $1
		   and exchange_segment = $2
		   and interval = $3
		   and timestamp >= extract(epoch from $4::text::date)::bigint
		   and timestamp <= extract(epoch from ($5::text::date + interval '1 day'))::bigint
		 order by timestamp`,
		securityID, exchangeSegment, interval, from, to,
	)
	if err != nil {
		http.Error(w, "query error", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	candles := []Candle{}
	for rows.Next() {
		var c Candle
		if err := rows.Scan(&c.Timestamp, &c.Open, &c.High, &c.Low, &c.Close, &c.Volume); err != nil {
			http.Error(w, "scan error", http.StatusInternalServerError)
			return
		}
		candles = append(candles, c)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(candles)
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /chart/candles", h.Candles)
}
