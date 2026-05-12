package nifty500

import (
	"database/sql"
	"encoding/json"
	"net/http"
)

type Handler struct {
	DB *sql.DB
}

type stockResult struct {
	Symbol      string `json:"symbol"`
	CompanyName string `json:"company_name"`
	Industry    string `json:"industry"`
	Series      string `json:"series"`
	ISIN        string `json:"isin"`
}

func (h *Handler) List(w http.ResponseWriter, r *http.Request) {
	rows, err := h.DB.Query(`
		select symbol, company_name, industry, series, isin
		from nifty500_constituents
		order by symbol
	`)
	if err != nil {
		http.Error(w, "DB error", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	results := []stockResult{}
	for rows.Next() {
		var s stockResult
		if err := rows.Scan(&s.Symbol, &s.CompanyName, &s.Industry, &s.Series, &s.ISIN); err != nil {
			continue
		}
		results = append(results, s)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(results)
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /nifty500", h.List)
}
