package candles

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

type candleResp struct {
	Open         []float64 `json:"open"`
	High         []float64 `json:"high"`
	Low          []float64 `json:"low"`
	Close        []float64 `json:"close"`
	Volume       []float64 `json:"volume"`
	Timestamp    []float64 `json:"timestamp"`
	OpenInterest []float64 `json:"open_interest"`
}

func MapSegment(seg string) (dhanSeg, instrument string) {
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

func IntervalMinutes(interval string) int {
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
		return 0
	}
}

func FetchAndStore(db *sql.DB, dhanBaseURL, clientID, accessToken, secID, seg, interval, fromDate, toDate string, updateOnConflict ...bool) error {
	doUpdate := len(updateOnConflict) > 0 && updateOnConflict[0]
	dhanSeg, instrType := MapSegment(seg)
	mins := IntervalMinutes(interval)

	var chunks [][2]string
	if mins == 0 {
		chunks = [][2]string{{fromDate, toDate}}
	} else {
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
				"oi":              true,
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
				"oi":              true,
				"fromDate":        chunk[0],
				"toDate":          chunk[1],
			}
			body, _ = json.Marshal(payload)
			endpoint = "/charts/intraday"
		}

		req, _ := http.NewRequest("POST", dhanBaseURL+endpoint, bytes.NewReader(body))
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

		var cr candleResp
		if err := json.Unmarshal(respBody, &cr); err != nil {
			return fmt.Errorf("parse candles: %w", err)
		}

		if err := upsert(db, secID, seg, interval, cr, doUpdate); err != nil {
			return err
		}
	}

	return nil
}

func upsert(db *sql.DB, secID, seg, interval string, c candleResp, update bool) error {
	tx, err := db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	conflict := "on conflict do nothing"
	if update {
		conflict = "on conflict (security_id, exchange_segment, interval, timestamp) do update set open=excluded.open, high=excluded.high, low=excluded.low, close=excluded.close, volume=excluded.volume, oi=excluded.oi"
	}

	stmt, err := tx.Prepare(`
		insert into candles (security_id, exchange_segment, interval, timestamp, open, high, low, close, volume, oi)
		values ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8, $9, $10)
		` + conflict)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for i := range c.Close {
		var oi int64
		if i < len(c.OpenInterest) {
			oi = int64(c.OpenInterest[i])
		}
		_, err := stmt.Exec(secID, seg, interval,
			int64(c.Timestamp[i]),
			c.Open[i], c.High[i], c.Low[i], c.Close[i], int64(c.Volume[i]), oi)
		if err != nil {
			return err
		}
	}

	return tx.Commit()
}
