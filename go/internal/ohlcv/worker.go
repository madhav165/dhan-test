package ohlcv

import (
	"context"
	"database/sql"
	"log"
	"os"
	"sync"
	"time"

	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/candles"
	"golang.org/x/time/rate"
)

type stock struct{ secID, seg string }

type Worker struct {
	DB          *sql.DB
	EncKey      []byte
	DhanBaseURL string
	Limiter     *rate.Limiter
}

func (w *Worker) Start() {
	userID := os.Getenv("OHLCV_USER_ID")
	if userID == "" {
		log.Println("ohlcv: OHLCV_USER_ID not set, scheduler disabled")
		return
	}

	// Run immediately on boot if needed, then schedule daily
	w.runOnce(userID)

	for {
		d := next4PMIST()
		log.Printf("ohlcv: next run in %v", d.Round(time.Minute))
		time.Sleep(d)
		w.runOnce(userID)
	}
}

func next4PMIST() time.Duration {
	ist := time.FixedZone("Asia/Kolkata", 5*60*60+30*60)
	now := time.Now().In(ist)
	next := time.Date(now.Year(), now.Month(), now.Day(), 16, 0, 0, 0, ist)
	if !next.After(now) {
		next = next.Add(24 * time.Hour)
	}
	return time.Until(next)
}

func (w *Worker) runOnce(userID string) {
	clientID, accessToken, err := broker.GetToken(w.DB, w.EncKey, userID)
	if err != nil {
		log.Printf("ohlcv: failed to get token for user %s: %v", userID, err)
		return
	}

	// Fetch all NSE_E stocks from nifty500_constituents joined with instruments
	rows, err := w.DB.Query(`
		select i.security_id, i.exchange_segment
		from nifty500_constituents n
		join instruments i on (i.trading_symbol = n.symbol or i.custom_symbol = n.symbol)
		where i.exchange_segment = 'NSE_E'
		order by n.symbol
	`)
	if err != nil {
		log.Printf("ohlcv: query instruments error: %v", err)
		return
	}
	defer rows.Close()

	var stocks []stock
	for rows.Next() {
		var s stock
		if err := rows.Scan(&s.secID, &s.seg); err != nil {
			continue
		}
		stocks = append(stocks, s)
	}
	rows.Close()

	if len(stocks) == 0 {
		log.Println("ohlcv: no stocks to process")
		return
	}

	// Create jobs for stocks that don't already have a pending job
	for _, s := range stocks {
		var exists int
		w.DB.QueryRow(`
			select count(*) from ohlcv_jobs
			where security_id = $1 and exchange_segment = $2 and status = 'pending'
		`, s.secID, s.seg).Scan(&exists)
		if exists > 0 {
			continue
		}

		_, err := w.DB.Exec(`
			insert into ohlcv_jobs (security_id, exchange_segment, status)
			values ($1, $2, 'pending')
		`, s.secID, s.seg)
		if err != nil {
			log.Printf("ohlcv: failed to create job for %s: %v", s.secID, err)
		}
	}

	// Worker pool: 5 goroutines
	const numWorkers = 5
	jobs := make(chan stock, numWorkers)
	var wg sync.WaitGroup

	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for s := range jobs {
				w.processStock(s, clientID, accessToken)
			}
		}()
	}

	// Feed workers
	for _, s := range stocks {
		// Only process if there's a pending job for this stock
		var jobID string
		err := w.DB.QueryRow(`
			select id from ohlcv_jobs
			where security_id = $1 and exchange_segment = $2 and status = 'pending'
			order by created_at
			limit 1
		`, s.secID, s.seg).Scan(&jobID)
		if err != nil {
			continue // no pending job
		}
		jobs <- s
	}
	close(jobs)
	wg.Wait()

	log.Printf("ohlcv: completed run for %d stocks", len(stocks))
}

func (w *Worker) processStock(s stock, clientID, accessToken string) {
	ctx := context.Background()

	// Determine date range
	var maxDate sql.NullTime
	w.DB.QueryRow(`
		select max(timestamp) from candles
		where security_id = $1 and exchange_segment = $2 and interval = '1d'
	`, s.secID, s.seg).Scan(&maxDate)

	var fromDate, toDate string
	today := time.Now().Format("2006-01-02")

	if !maxDate.Valid {
		// No data: fetch 10 years
		fromDate = time.Now().AddDate(-10, 0, 0).Format("2006-01-02")
		toDate = today
		log.Printf("ohlcv: first load for %s %s %s–%s", s.secID, s.seg, fromDate, toDate)
	} else {
		// Incremental: from max date to today
		fromDate = maxDate.Time.Format("2006-01-02")
		if fromDate >= today {
			log.Printf("ohlcv: up to date for %s %s", s.secID, s.seg)
			w.markDone(s.secID, s.seg, "")
			return
		}
		toDate = today
		log.Printf("ohlcv: incremental for %s %s %s–%s", s.secID, s.seg, fromDate, toDate)
	}

	if err := candles.FetchAndStore(ctx, w.DB, w.DhanBaseURL, clientID, accessToken, s.secID, s.seg, "1d", fromDate, toDate, w.Limiter); err != nil {
		log.Printf("ohlcv: fetch failed for %s: %v", s.secID, err)
		w.markDone(s.secID, s.seg, err.Error())
		return
	}

	w.markDone(s.secID, s.seg, "")
}

func (w *Worker) markDone(secID, seg, errMsg string) {
	if errMsg != "" {
		w.DB.Exec(`
			update ohlcv_jobs set status = 'failed', error = $1, updated_at = now()
			where security_id = $2 and exchange_segment = $3 and status = 'pending'
		`, errMsg, secID, seg)
	} else {
		w.DB.Exec(`
			update ohlcv_jobs set status = 'done', updated_at = now()
			where security_id = $1 and exchange_segment = $2 and status = 'pending'
		`, secID, seg)
	}
}
