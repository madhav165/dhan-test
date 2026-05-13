package ohlcv

import (
	"encoding/json"
	"net/http"
	"os"
	"strconv"
)

type StatsResponse struct {
	Summary map[string]int            `json:"summary"`
	Failed  map[string]int            `json:"failed"`
}

func (w *Worker) HandleStats(rw http.ResponseWriter, r *http.Request) {
	adminUserID := os.Getenv("OHLCV_USER_ID")
	if r.Header.Get("X-User-ID") != adminUserID {
		http.Error(rw, "Forbidden", http.StatusForbidden)
		return
	}

	// Status summary
	summaryRows, err := w.DB.Query(`
		select status, count(*)::int from ohlcv_jobs group by status
	`)
	if err != nil {
		http.Error(rw, "DB error", http.StatusInternalServerError)
		return
	}
	defer summaryRows.Close()

	summary := map[string]int{}
	for summaryRows.Next() {
		var status string
		var count int
		if err := summaryRows.Scan(&status, &count); err == nil {
			summary[status] = count
		}
	}
	summaryRows.Close()

	// Failed breakdown by error type
	failedRows, err := w.DB.Query(`
		select
			case
				when error ilike '%status 429%' then 'rate_limited'
				when error ilike '%status 400%' then 'no_data'
				when error ilike '%token%'      then 'token_error'
				else                                 'other'
			end as error_type,
			count(*)::int
		from ohlcv_jobs
		where status = 'failed'
		group by error_type
	`)
	if err != nil {
		http.Error(rw, "DB error", http.StatusInternalServerError)
		return
	}
	defer failedRows.Close()

	failed := map[string]int{}
	for failedRows.Next() {
		var errType string
		var count int
		if err := failedRows.Scan(&errType, &count); err == nil {
			failed[errType] = count
		}
	}

	rw.Header().Set("Content-Type", "application/json")
	json.NewEncoder(rw).Encode(StatsResponse{Summary: summary, Failed: failed})
}

type StockRow struct {
	Symbol      string `json:"symbol"`
	CompanyName string `json:"company_name"`
	Industry    string `json:"industry"`
	StartDate   string `json:"start_date"`
	EndDate     string `json:"end_date"`
	Candles     int    `json:"candles"`
}

type StocksResponse struct {
	Stocks     []StockRow `json:"stocks"`
	Industries []string   `json:"industries"`
	Total      int        `json:"total"`
	Page       int        `json:"page"`
	PageSize   int        `json:"page_size"`
}

const pageSize = 50

func (w *Worker) HandleStocks(rw http.ResponseWriter, r *http.Request) {
	q := r.URL.Query().Get("q")
	industry := r.URL.Query().Get("industry")
	page, _ := strconv.Atoi(r.URL.Query().Get("page"))
	if page < 1 {
		page = 1
	}
	offset := (page - 1) * pageSize

	// Build WHERE clause
	args := []any{}
	where := "where i.exchange_segment = 'NSE_E'"
	if q != "" {
		args = append(args, "%"+q+"%")
		where += " and (n.symbol ilike $" + strconv.Itoa(len(args)) +
			" or n.company_name ilike $" + strconv.Itoa(len(args)) + ")"
	}
	if industry != "" {
		args = append(args, industry)
		where += " and n.industry = $" + strconv.Itoa(len(args))
	}

	baseQuery := `
		from nse500_extended n
		join instruments i on (i.trading_symbol = n.symbol or i.custom_symbol = n.symbol)
		join candles c on c.security_id = i.security_id and c.exchange_segment = 'NSE_E' and c.interval = '1d'
		` + where + `
		group by n.symbol, n.company_name, n.industry`

	// Total count
	countArgs := append([]any{}, args...)
	var total int
	if err := w.DB.QueryRow(`select count(*) from (select n.symbol `+baseQuery+`) t`, countArgs...).Scan(&total); err != nil {
		http.Error(rw, "DB error", http.StatusInternalServerError)
		return
	}

	// Paginated rows
	pageArgs := append(args, pageSize, offset)
	limitIdx := strconv.Itoa(len(pageArgs) - 1)
	offsetIdx := strconv.Itoa(len(pageArgs))
	rows, err := w.DB.Query(`
		select n.symbol, n.company_name, coalesce(n.industry, ''),
		       min(c.timestamp)::date::text, max(c.timestamp)::date::text,
		       count(*)::int
		`+baseQuery+`
		order by n.symbol
		limit $`+limitIdx+` offset $`+offsetIdx,
		pageArgs...)
	if err != nil {
		http.Error(rw, "DB error", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	stocks := []StockRow{}
	for rows.Next() {
		var s StockRow
		if err := rows.Scan(&s.Symbol, &s.CompanyName, &s.Industry,
			&s.StartDate, &s.EndDate, &s.Candles); err != nil {
			continue
		}
		stocks = append(stocks, s)
	}
	rows.Close()

	// Distinct industries for filter dropdown
	indRows, err := w.DB.Query(`
		select distinct n.industry from nse500_extended n
		where n.industry is not null order by n.industry
	`)
	if err != nil {
		http.Error(rw, "DB error", http.StatusInternalServerError)
		return
	}
	defer indRows.Close()

	industries := []string{}
	for indRows.Next() {
		var ind string
		if err := indRows.Scan(&ind); err == nil {
			industries = append(industries, ind)
		}
	}

	rw.Header().Set("Content-Type", "application/json")
	json.NewEncoder(rw).Encode(StocksResponse{
		Stocks:     stocks,
		Industries: industries,
		Total:      total,
		Page:       page,
		PageSize:   pageSize,
	})
}

func (w *Worker) RegisterAdminRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /admin/ohlcv", w.HandleStats)
	mux.HandleFunc("GET /admin/ohlcv/ws", w.HandleStatusWS)
	mux.HandleFunc("GET /ohlcv/stocks", w.HandleStocks)
	mux.HandleFunc("POST /internal/ohlcv-trigger", w.HandleTrigger)
}

func (w *Worker) HandleTrigger(rw http.ResponseWriter, r *http.Request) {
	userID := os.Getenv("OHLCV_USER_ID")
	if userID == "" {
		http.Error(rw, "disabled", http.StatusServiceUnavailable)
		return
	}
	go w.createJobs(userID)
	rw.WriteHeader(http.StatusNoContent)
}
