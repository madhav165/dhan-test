package instrument

import (
	"database/sql"
	"encoding/json"
	"net/http"
)

type Handler struct {
	DB *sql.DB
}

type instrumentResult struct {
	SecurityID      string `json:"security_id"`
	ExchangeSegment string `json:"exchange_segment"`
	TradingSymbol   string `json:"trading_symbol"`
	CustomSymbol    string `json:"custom_symbol"`
	InstrumentType  string `json:"instrument_type"`
}

func (h *Handler) Search(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query().Get("q")
	if len(q) < 1 {
		w.Header().Set("Content-Type", "application/json")
		w.Write([]byte("[]"))
		return
	}

	rows, err := h.DB.Query(`
		select security_id, exchange_segment, trading_symbol, coalesce(custom_symbol, ''), instrument_type
		from instruments
		where trading_symbol ilike $1 or custom_symbol ilike $1
		order by
			case when trading_symbol ilike $2 then 0 else 1 end,
			trading_symbol
		limit 20
	`, "%"+q+"%", q+"%")
	if err != nil {
		http.Error(w, "DB error", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	results := []instrumentResult{}
	for rows.Next() {
		var i instrumentResult
		if err := rows.Scan(&i.SecurityID, &i.ExchangeSegment, &i.TradingSymbol, &i.CustomSymbol, &i.InstrumentType); err != nil {
			continue
		}
		results = append(results, i)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(results)
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /instruments/search", h.Search)
}
