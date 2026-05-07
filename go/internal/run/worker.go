package run

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"time"

	"github.com/madhav165/dhan-test/go/internal/broker"
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
		if err := w.fetchAndStore(clientID, accessToken, i.secID, i.seg, interval, fromDate, toDate); err != nil {
			w.fail(jobID, fmt.Sprintf("fetch %s: %v", i.secID, err))
			return
		}
	}

	w.DB.Exec("update run_jobs set status='ready', updated_at=now() where id=$1", jobID)
	log.Printf("run worker: job %s ready", jobID)
}

func mapSegment(seg string) (dhanSeg, instrument string) {
	switch seg {
	case "NSE_E":
		return "NSE_EQ", "EQUITY"
	case "BSE_E":
		return "BSE_EQ", "EQUITY"
	case "NSE_I":
		return "IDX_I", "INDEX"
	default:
		return "NSE_EQ", "EQUITY"
	}
}

func intervalMinutes(interval string) int {
	switch interval {
	case "1min":
		return 1
	case "5min":
		return 5
	case "15min":
		return 15
	case "25min":
		return 25
	case "60min":
		return 60
	default:
		return 0 // daily
	}
}

type candleResp struct {
	Open      []float64 `json:"open"`
	High      []float64 `json:"high"`
	Low       []float64 `json:"low"`
	Close     []float64 `json:"close"`
	Volume    []int64   `json:"volume"`
	Timestamp []int64   `json:"timestamp"`
}

func (w *Worker) fetchAndStore(clientID, accessToken, secID, seg, interval, fromDate, toDate string) error {
	dhanSeg, instrType := mapSegment(seg)
	mins := intervalMinutes(interval)

	var chunks [][2]string
	if mins == 0 {
		chunks = [][2]string{{fromDate, toDate}}
	} else {
		// paginate in 90-day chunks
		from, err := time.Parse("2006-01-02", fromDate)
		if err != nil {
			return err
		}
		to, err := time.Parse("2006-01-02", toDate)
		if err != nil {
			return err
		}
		for cur := from; !cur.After(to); {
			end := cur.AddDate(0, 0, 89)
			if end.After(to) {
				end = to
			}
			chunks = append(chunks, [2]string{cur.Format("2006-01-02"), end.Format("2006-01-02")})
			cur = end.AddDate(0, 0, 1)
		}
	}

	for _, chunk := range chunks {
		var body []byte
		var endpoint string
		if mins == 0 {
			payload := map[string]any{
				"securityId":      secID,
				"exchangeSegment": dhanSeg,
				"instrument":      instrType,
				"expiryCode":      0,
				"fromDate":        chunk[0],
				"toDate":          chunk[1],
			}
			body, _ = json.Marshal(payload)
			endpoint = "/charts/historical"
		} else {
			payload := map[string]any{
				"securityId":      secID,
				"exchangeSegment": dhanSeg,
				"instrument":      instrType,
				"interval":        fmt.Sprintf("%d", mins),
				"fromDate":        chunk[0],
				"toDate":          chunk[1],
			}
			body, _ = json.Marshal(payload)
			endpoint = "/charts/intraday"
		}

		req, _ := http.NewRequest("POST", w.DhanBaseURL+endpoint, bytes.NewReader(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("access-token", accessToken)
		req.Header.Set("client-id", clientID)

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return fmt.Errorf("dhan request: %w", err)
		}
		respBody, _ := io.ReadAll(resp.Body)
		resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			return fmt.Errorf("dhan %s status %d: %s", endpoint, resp.StatusCode, string(respBody))
		}

		var candles candleResp
		if err := json.Unmarshal(respBody, &candles); err != nil {
			return fmt.Errorf("parse candles: %w", err)
		}

		if err := w.upsertCandles(secID, seg, interval, candles); err != nil {
			return err
		}
	}

	return nil
}

func (w *Worker) upsertCandles(secID, seg, interval string, c candleResp) error {
	tx, err := w.DB.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	stmt, err := tx.Prepare(`
		insert into candles (security_id, exchange_segment, interval, timestamp, open, high, low, close, volume)
		values ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8, $9)
		on conflict do nothing`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for i := range c.Close {
		_, err := stmt.Exec(secID, seg, interval,
			c.Timestamp[i],
			c.Open[i], c.High[i], c.Low[i], c.Close[i], c.Volume[i])
		if err != nil {
			return err
		}
	}

	return tx.Commit()
}

func (w *Worker) fail(jobID, msg string) {
	log.Printf("run worker: job %s failed: %s", jobID, msg)
	w.DB.Exec("update run_jobs set status='failed', error=$1, updated_at=now() where id=$2", msg, jobID)
}
