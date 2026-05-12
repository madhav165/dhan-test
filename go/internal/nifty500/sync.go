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

func todayIST() time.Time {
	return time.Now().In(ist)
}

func nextSunday9AM() time.Duration {
	now := time.Now().In(ist)
	// Calculate days until next Sunday (0 = Sunday)
	daysUntilSunday := (7 - int(now.Weekday())) % 7
	if daysUntilSunday == 0 {
		// If today is Sunday, schedule for next Sunday
		daysUntilSunday = 7
	}
	next := time.Date(now.Year(), now.Month(), now.Day(), 9, 0, 0, 0, ist)
	next = next.AddDate(0, 0, daysUntilSunday)
	return time.Until(next)
}

func alreadySyncedThisWeek(db *sql.DB) bool {
	var count int
	// Check if synced in last 6 days (weekly schedule)
	since := time.Now().UTC().Add(-6 * 24 * time.Hour)
	err := db.QueryRow(`select count(*) from nifty500_constituents where last_synced > $1`, since).Scan(&count)
	return err == nil && count > 0
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

	tx, err := db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	// Clear and re-insert for simplicity (NIFTY 500 list changes infrequently)
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

	// Header: Company Name, Industry, Symbol, Series, ISIN Code
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

func syncExtended(db *sql.DB) error {
	_, err := db.Exec(`
		insert into nse500_extended (symbol, company_name, industry, series, isin, last_synced)
		select symbol, company_name, industry, series, isin, now()
		from nifty500_constituents
		on conflict (symbol) do update set
			company_name = excluded.company_name,
			industry = excluded.industry,
			series = excluded.series,
			isin = excluded.isin,
			last_synced = now()
	`)
	if err != nil {
		return fmt.Errorf("upsert: %w", err)
	}
	return nil
}

func RunScheduler(db *sql.DB) {
	if !alreadySyncedThisWeek(db) {
		log.Println("nifty500: syncing now")
		if err := downloadAndSync(db); err != nil {
			log.Printf("nifty500: sync failed: %v", err)
		} else {
			if err := syncExtended(db); err != nil {
				log.Printf("nse500_extended: upsert failed: %v", err)
			} else {
				log.Println("nse500_extended: upserted")
			}
			log.Println("nifty500: sync complete")
		}
	} else {
		log.Println("nifty500: already synced this week, skipping")
	}

	for {
		d := nextSunday9AM()
		log.Printf("nifty500: next sync in %v", d.Round(time.Minute))
		time.Sleep(d)

		log.Println("nifty500: syncing")
		if err := downloadAndSync(db); err != nil {
			log.Printf("nifty500: sync failed: %v", err)
		} else {
			if err := syncExtended(db); err != nil {
				log.Printf("nse500_extended: upsert failed: %v", err)
			} else {
				log.Println("nse500_extended: upserted")
			}
			log.Println("nifty500: sync complete")
		}
	}
}
