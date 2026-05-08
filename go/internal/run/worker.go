package run

import (
	"database/sql"
	"fmt"
	"log"
	"time"

	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/candles"
)

type Worker struct {
	DB          *sql.DB
	EncKey      []byte
	DhanBaseURL string
}

func (w *Worker) Start() {
	for {
		w.poll()
		time.Sleep(3 * time.Second)
	}
}

func (w *Worker) poll() {
	rows, err := w.DB.Query(`
		select j.id, j.run_id, r.interval, r.from_date::text, r.to_date::text, s.user_id::text
		from run_jobs j
		join backtest_runs r on r.id = j.run_id
		join strategies s on s.id = r.strategy_id
		where j.status = 'pending'
		order by j.created_at
		limit 1`)
	if err != nil {
		log.Printf("run worker: query error: %v", err)
		return
	}
	defer rows.Close()

	for rows.Next() {
		var jobID, runID, interval, fromDate, toDate, userID string
		if err := rows.Scan(&jobID, &runID, &interval, &fromDate, &toDate, &userID); err != nil {
			log.Printf("run worker: scan error: %v", err)
			continue
		}
		w.processJob(jobID, runID, interval, fromDate, toDate, userID)
	}
}

func (w *Worker) processJob(jobID, runID, interval, fromDate, toDate, userID string) {
	instrRows, err := w.DB.Query(
		"select security_id, exchange_segment from backtest_run_instruments where run_id = $1", runID)
	if err != nil {
		w.fail(jobID, fmt.Sprintf("get instruments: %v", err))
		return
	}
	defer instrRows.Close()

	type inst struct{ secID, seg string }
	var instruments []inst
	for instrRows.Next() {
		var i inst
		instrRows.Scan(&i.secID, &i.seg)
		instruments = append(instruments, i)
	}
	instrRows.Close()

	if len(instruments) == 0 {
		w.fail(jobID, "no instruments")
		return
	}

	clientID, accessToken, err := broker.GetToken(w.DB, w.EncKey, userID)
	if err != nil {
		w.fail(jobID, fmt.Sprintf("get dhan token: %v", err))
		return
	}

	for _, i := range instruments {
		var count int
		w.DB.QueryRow(`
			select count(*) from candles
			where security_id=$1 and exchange_segment=$2 and interval=$3
			and timestamp::date between $4::date and $5::date`,
			i.secID, i.seg, interval, fromDate, toDate,
		).Scan(&count)

		if count > 0 {
			log.Printf("run worker: candles exist for %s %s", i.secID, i.seg)
			continue
		}

		log.Printf("run worker: fetching candles for %s %s %s %s–%s", i.secID, i.seg, interval, fromDate, toDate)
		if err := candles.FetchAndStore(w.DB, w.DhanBaseURL, clientID, accessToken, i.secID, i.seg, interval, fromDate, toDate); err != nil {
			w.fail(jobID, fmt.Sprintf("fetch %s: %v", i.secID, err))
			return
		}
	}

	w.DB.Exec("update run_jobs set status='ready', updated_at=now() where id=$1", jobID)
	log.Printf("run worker: job %s ready", jobID)
}

func (w *Worker) fail(jobID, msg string) {
	log.Printf("run worker: job %s failed: %s", jobID, msg)
	w.DB.Exec("update run_jobs set status='failed', error=$1, updated_at=now() where id=$2", msg, jobID)
}
