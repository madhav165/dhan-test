package ohlcv

import (
	"encoding/json"
	"net/http"
	"os"
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

func (w *Worker) RegisterAdminRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /admin/ohlcv", w.HandleStats)
}
