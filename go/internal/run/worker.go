package run

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
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
	for {
		w.pollRuns()
		w.pollRLJobs()
		time.Sleep(3 * time.Second)
	}
}

func (w *Worker) pollRuns() {
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
		w.processRunJob(jobID, runID, interval, fromDate, toDate, userID)
	}
}

func (w *Worker) pollRLJobs() {
	rows, err := w.DB.Query(`
		select j.id, j.strategy_id, s.rl_config, s.user_id::text
		from rl_jobs j
		join strategies s on s.id = j.strategy_id
		where j.status = 'pending'
		order by j.created_at
		limit 1`)
	if err != nil {
		log.Printf("run worker: rl query error: %v", err)
		return
	}
	defer rows.Close()

	for rows.Next() {
		var jobID, strategyID, userID string
		var rlConfigRaw []byte
		if err := rows.Scan(&jobID, &strategyID, &rlConfigRaw, &userID); err != nil {
			log.Printf("run worker: rl scan error: %v", err)
			continue
		}
		w.processRLJob(jobID, strategyID, rlConfigRaw, userID)
	}
}

func (w *Worker) processRunJob(jobID, runID, interval, fromDate, toDate, userID string) {
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
		if err := candles.FetchAndStore(context.Background(), w.DB, w.DhanBaseURL, clientID, accessToken, i.secID, i.seg, interval, fromDate, toDate, w.DataRL.Get(userID)); err != nil {
			w.fail(jobID, fmt.Sprintf("fetch %s: %v", i.secID, err))
			return
		}
	}

	w.DB.Exec("update run_jobs set status='ready', updated_at=now() where id=$1", jobID)
	log.Printf("run worker: job %s ready", jobID)
}

func (w *Worker) processRLJob(jobID, strategyID string, rlConfigRaw []byte, userID string) {
	var rlConfig map[string]interface{}
	if err := json.Unmarshal(rlConfigRaw, &rlConfig); err != nil {
		w.failRL(jobID, fmt.Sprintf("parse rl_config: %v", err))
		return
	}

	interval, _ := rlConfig["interval"].(string)
	trainFrom, _ := rlConfig["train_from"].(string)
	trainTo, _ := rlConfig["train_to"].(string)
	secID, _ := rlConfig["security_id"].(string)
	seg, _ := rlConfig["exchange_segment"].(string)

	if interval == "" || trainFrom == "" || trainTo == "" || secID == "" || seg == "" {
		w.failRL(jobID, "missing required fields in rl_config")
		return
	}

	clientID, accessToken, err := broker.GetToken(w.DB, w.EncKey, userID)
	if err != nil {
		w.failRL(jobID, fmt.Sprintf("get dhan token: %v", err))
		return
	}

	var count int
	w.DB.QueryRow(`
		select count(*) from candles
		where security_id=$1 and exchange_segment=$2 and interval=$3
		and timestamp::date between $4::date and $5::date`,
		secID, seg, interval, trainFrom, trainTo,
	).Scan(&count)

	if count == 0 {
		log.Printf("run worker: fetching candles for RL %s %s %s %s–%s", secID, seg, interval, trainFrom, trainTo)
		if err := candles.FetchAndStore(context.Background(), w.DB, w.DhanBaseURL, clientID, accessToken, secID, seg, interval, trainFrom, trainTo, w.DataRL.Get(userID)); err != nil {
			w.failRL(jobID, fmt.Sprintf("fetch %s: %v", secID, err))
			return
		}
	} else {
		log.Printf("run worker: candles exist for RL %s %s", secID, seg)
	}

	w.DB.Exec("update rl_jobs set status='training', updated_at=now() where id=$1", jobID)
	log.Printf("run worker: rl job %s ready for training", jobID)
}

func (w *Worker) failRL(jobID, msg string) {
	log.Printf("run worker: rl job %s failed: %s", jobID, msg)
	w.DB.Exec("update rl_jobs set status='failed', error=$1, updated_at=now() where id=$2", msg, jobID)
}

func (w *Worker) fail(jobID, msg string) {
	log.Printf("run worker: job %s failed: %s", jobID, msg)
	w.DB.Exec("update run_jobs set status='failed', error=$1, updated_at=now() where id=$2", msg, jobID)
}
