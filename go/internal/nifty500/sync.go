package nifty500

import (
	"database/sql"
	"encoding/csv"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"
)

const nifty500CSVURL = "https://archives.nseindia.com/content/indices/ind_nifty500list.csv"

var ist = time.FixedZone("Asia/Kolkata", 5*60*60+30*60)

func nextMonthFirst9AM() time.Duration {
	now := time.Now().In(ist)
	next := time.Date(now.Year(), now.Month()+1, 1, 9, 0, 0, 0, ist)
	// If it's already the 1st at 9AM or later, schedule for next month
	if now.Month() == next.Month() && now.Hour() >= 9 {
		next = next.AddDate(0, 1, 0)
	}
	return time.Until(next)
}

func alreadySyncedThisMonth(db *sql.DB) bool {
	var count int
	since := time.Now().UTC().Add(-31 * 24 * time.Hour)
	err := db.QueryRow(`select count(*) from nifty500_constituents where last_synced > $1`, since).Scan(&count)
	return err == nil && count > 0
}

func getLatestSnapshotSymbols(db *sql.DB) ([]string, error) {
	var latestDate sql.NullString
	if err := db.QueryRow(`select max(snapshot_date)::text from nifty500_snapshots`).Scan(&latestDate); err != nil {
		return nil, err
	}
	if !latestDate.Valid {
		return nil, nil
	}
	rows, err := db.Query(`select symbol from nifty500_snapshots where snapshot_date = $1 order by symbol`, latestDate.String)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var symbols []string
	for rows.Next() {
		var s string
		if err := rows.Scan(&s); err != nil {
			continue
		}
		symbols = append(symbols, s)
	}
	return symbols, rows.Err()
}

func downloadAndSync(db *sql.DB) error {
	resp, err := http.Get(nifty500CSVURL)
	if err != nil {
		return fmt.Errorf("download: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download: status %d", resp.StatusCode)
	}

	r := csv.NewReader(resp.Body)
	records, err := r.ReadAll()
	if err != nil {
		return fmt.Errorf("parse csv: %w", err)
	}

	if len(records) < 2 {
		return fmt.Errorf("csv has no data rows")
	}

	// Collect new symbols for comparison
	var newSymbols []string
	for _, row := range records[1:] {
		if len(row) < 5 {
			continue
		}
		symbol := strings.TrimSpace(row[2])
		if symbol != "" {
			newSymbols = append(newSymbols, symbol)
		}
	}

	// Check if symbols changed vs latest snapshot
	oldSymbols, err := getLatestSnapshotSymbols(db)
	if err != nil {
		log.Printf("nifty500: check latest snapshot: %v (proceeding with sync)", err)
	} else if symbolsEqual(newSymbols, oldSymbols) {
		log.Println("nifty500: no change from latest snapshot, updating last_synced only")
		if _, err := db.Exec(`update nifty500_constituents set last_synced = now()`); err != nil {
			return fmt.Errorf("update last_synced: %w", err)
		}
		return nil
	}

	tx, err := db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	// Archive current live constituents into snapshots before wiping
	if _, err := tx.Exec(`
		insert into nifty500_snapshots (snapshot_date, symbol, company_name, industry, series, isin)
		select current_date - interval '1 month', symbol, company_name, industry, series, isin
		from nifty500_constituents
		on conflict (symbol, snapshot_date) do update set
			company_name = excluded.company_name,
			industry = excluded.industry,
			series = excluded.series,
			isin = excluded.isin
	`); err != nil {
		return fmt.Errorf("archive to snapshots: %w", err)
	}

	// Clear and re-insert
	if _, err := tx.Exec(`delete from nifty500_constituents`); err != nil {
		return err
	}

	stmt, err := tx.Prepare(`
		insert into nifty500_constituents (symbol, company_name, industry, series, isin, last_synced)
		values ($1, $2, $3, $4, $5, now())
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, row := range records[1:] {
		if len(row) < 5 {
			continue
		}
		companyName := strings.TrimSpace(row[0])
		industry := strings.TrimSpace(row[1])
		symbol := strings.TrimSpace(row[2])
		series := strings.TrimSpace(row[3])
		isin := strings.TrimSpace(row[4])

		if symbol == "" {
			continue
		}

		if _, err := stmt.Exec(symbol, companyName, industry, series, isin); err != nil {
			return err
		}
	}

	return tx.Commit()
}

func symbolsEqual(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func RunScheduler(db *sql.DB) {
	if !alreadySyncedThisMonth(db) {
		log.Println("nifty500: syncing now")
		if err := downloadAndSync(db); err != nil {
			log.Printf("nifty500: sync failed: %v", err)
		} else {
			log.Println("nifty500: sync complete")
		}
	} else {
		log.Println("nifty500: already synced this month, skipping")
	}

	for {
		d := nextMonthFirst9AM()
		log.Printf("nifty500: next sync in %v", d.Round(time.Hour))
		time.Sleep(d)

		log.Println("nifty500: syncing")
		if err := downloadAndSync(db); err != nil {
			log.Printf("nifty500: sync failed: %v", err)
		} else {
			log.Println("nifty500: sync complete")
		}
	}
}
