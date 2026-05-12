package ohlcv

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/candles"
	"github.com/madhav165/dhan-test/go/internal/ratelimit"
)

type Worker struct {
	DB          *sql.DB
	EncKey      []byte
	DhanBaseURL string
	DataRL      *ratelimit.Store
}

func (w *Worker) Start() {
	userID := os.Getenv("OHLCV_USER_ID")
	if userID == "" {
		log.Println("ohlcv: OHLCV_USER_ID not set, scheduler disabled")
		return
	}

	// Run immediately on boot if needed, then schedule daily
	w.createJobs(userID)

	// Start 5 workers with semaphore
	sem := make(chan struct{}, 5)
	for i := 0; i < 5; i++ {
		go w.workerLoop(userID, sem)
	}

	for {
		d := next4PMIST()
		log.Printf("ohlcv: next job creation in %v", d.Round(time.Minute))
		time.Sleep(d)
		w.createJobs(userID)
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

func (w *Worker) createJobs(userID string) {
	// Verify broker connection exists before creating jobs
	_, _, err := broker.GetToken(w.DB, w.EncKey, userID)
	if err != nil {
		log.Printf("ohlcv: no valid broker connection for user %s, skipping job creation: %v", userID, err)
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

	type stock struct{ secID, seg string }
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

	// TEMP: test with first stock only
	if len(stocks) > 0 {
		w.createJobsForStock(stocks[0].secID, stocks[0].seg)
		log.Printf("ohlcv: created jobs for 1 stock (TEST MODE)")
	}
}

func (w *Worker) createJobsForStock(secID, seg string) {
	// Determine date range
	var maxDate sql.NullTime
	w.DB.QueryRow(`
		select max(timestamp) from candles
		where security_id = $1 and exchange_segment = $2 and interval = '1d'
	`, secID, seg).Scan(&maxDate)

	today := time.Now().Format("2006-01-02")
	var fromDate, toDate string

	if !maxDate.Valid {
		// TEMP: test with 1000 days
		fromDate = time.Now().AddDate(0, 0, -1000).Format("2006-01-02")
		toDate = today
	} else {
		fromDate = maxDate.Time.Format("2006-01-02")
		if fromDate >= today {
			return // up to date
		}
		toDate = today
	}

	// Split into 90-day chunks
	from, _ := time.Parse("2006-01-02", fromDate)
	to, _ := time.Parse("2006-01-02", toDate)

	for cur := from; !cur.After(to); {
		end := cur.AddDate(0, 0, 89)
		if end.After(to) {
			end = to
		}

		chunkFrom := cur.Format("2006-01-02")
		chunkTo := end.Format("2006-01-02")

		// Skip if pending or done job already exists for this chunk
		var exists int
		w.DB.QueryRow(`
			select count(*) from ohlcv_jobs
			where security_id = $1 and exchange_segment = $2 and from_date = $3 and to_date = $4
			and status in ('pending', 'done')
		`, secID, seg, chunkFrom, chunkTo).Scan(&exists)
		if exists > 0 {
			cur = end.AddDate(0, 0, 1)
			continue
		}

		_, err := w.DB.Exec(`
			insert into ohlcv_jobs (security_id, exchange_segment, from_date, to_date, interval, status, retry_count, max_retries)
			values ($1, $2, $3, $4, '1d', 'pending', 0, 3)
			on conflict (security_id, exchange_segment, from_date, to_date) where status = 'pending' do nothing
		`, secID, seg, chunkFrom, chunkTo)
		if err != nil {
			log.Printf("ohlcv: failed to create job for %s %s–%s: %v", secID, chunkFrom, chunkTo, err)
		}

		cur = end.AddDate(0, 0, 1)
	}
}

func (w *Worker) workerLoop(userID string, sem chan struct{}) {
	for {
		sem <- struct{}{} // acquire slot

		job := w.claimJob()
		if job == nil {
			<-sem // release slot
			time.Sleep(5 * time.Second)
			continue
		}

		w.processJob(job, userID)
		<-sem // release slot
	}
}

type job struct {
	id              string
	securityID      string
	exchangeSegment string
	fromDate        string
	toDate          string
	interval        string
	retryCount      int
}

func (w *Worker) claimJob() *job {
	var j job
	err := w.DB.QueryRow(`
		update ohlcv_jobs
		set status = 'running', updated_at = now()
		where id = (
			select id from ohlcv_jobs
			where status = 'pending'
			order by created_at
			for update skip locked
			limit 1
		)
		returning id, security_id, exchange_segment, from_date::text, to_date::text, interval, retry_count
	`).Scan(&j.id, &j.securityID, &j.exchangeSegment, &j.fromDate, &j.toDate, &j.interval, &j.retryCount)
	if err != nil {
		return nil
	}
	return &j
}

func (w *Worker) processJob(j *job, userID string) {
	ctx := context.Background()

	clientID, accessToken, err := broker.GetToken(w.DB, w.EncKey, userID)
	if err != nil {
		log.Printf("ohlcv: failed to get token for job %s: %v", j.id, err)
		w.failJob(j.id, fmt.Sprintf("token: %v", err))
		return
	}

	log.Printf("ohlcv: processing job %s %s %s–%s", j.securityID, j.exchangeSegment, j.fromDate, j.toDate)
	if err := candles.FetchChunk(ctx, w.DB, w.DhanBaseURL, clientID, accessToken, j.securityID, j.exchangeSegment, j.interval, j.fromDate, j.toDate, w.DataRL.Get(userID)); err != nil {
		log.Printf("ohlcv: job %s failed: %v", j.id, err)
		w.failJob(j.id, err.Error())
		return
	}

	w.DB.Exec(`update ohlcv_jobs set status = 'done', updated_at = now() where id = $1`, j.id)
	log.Printf("ohlcv: job %s done", j.id)
}

func (w *Worker) failJob(jobID, errMsg string) {
	// Non-retryable: 400 errors (no data, bad input) — don't waste retries
	if strings.Contains(errMsg, "status 400") || strings.Contains(errMsg, "Input_Exception") {
		w.DB.Exec(`
			update ohlcv_jobs
			set status = 'failed', error = $1, updated_at = now()
			where id = $2
		`, errMsg, jobID)
		log.Printf("ohlcv: job %s permanently failed (no data)", jobID)
		return
	}

	// Retryable: 429, 5xx, network errors — set back to pending for re-queue
	w.DB.Exec(`
		update ohlcv_jobs
		set status = case when retry_count >= max_retries then 'failed' else 'pending' end,
		    error = case when retry_count >= max_retries then $1 else null end,
		    retry_count = retry_count + 1,
		    updated_at = now()
		where id = $2
	`, errMsg, jobID)
}
