package instrument

import (
	"database/sql"
	"encoding/csv"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"time"
)

const csvURL = "https://images.dhan.co/api-data/api-scrip-master.csv"

var ist = time.FixedZone("Asia/Kolkata", 5*60*60+30*60)

func todayIST() time.Time {
	return time.Now().In(ist)
}

func nextNineAMIST() time.Duration {
	now := time.Now().In(ist)
	next := time.Date(now.Year(), now.Month(), now.Day(), 9, 0, 0, 0, ist)
	if !next.After(now) {
		next = next.Add(24 * time.Hour)
	}
	return time.Until(next)
}

func alreadySyncedToday(db *sql.DB) bool {
	var count int
	today := todayIST().Format("2006-01-02")
	err := db.QueryRow(`select count(*) from instruments where last_updated = $1`, today).Scan(&count)
	return err == nil && count > 0
}

func download(db *sql.DB) error {
	resp, err := http.Get(csvURL)
	if err != nil {
		return fmt.Errorf("download: %w", err)
	}
	defer resp.Body.Close()

	r := csv.NewReader(resp.Body)
	records, err := r.ReadAll()
	if err != nil {
		return fmt.Errorf("parse csv: %w", err)
	}

	today := todayIST().Format("2006-01-02")

	tx, err := db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if _, err := tx.Exec(`delete from instruments`); err != nil {
		return err
	}

	stmt, err := tx.Prepare(`
		insert into instruments
			(security_id, exchange_segment, trading_symbol, custom_symbol, instrument_type, lot_size, tick_size, last_updated)
		values ($1, $2, $3, $4, $5, $6, $7, $8)
		on conflict (security_id, exchange_segment) do update
		set trading_symbol = excluded.trading_symbol,
		    custom_symbol  = excluded.custom_symbol,
		    instrument_type = excluded.instrument_type,
		    lot_size        = excluded.lot_size,
		    tick_size       = excluded.tick_size,
		    last_updated    = excluded.last_updated
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	// header: SEM_EXM_EXCH_ID,SEM_SEGMENT,SEM_SMST_SECURITY_ID,SEM_INSTRUMENT_NAME,
	//         SEM_EXPIRY_CODE,SEM_TRADING_SYMBOL,SEM_LOT_UNITS,SEM_CUSTOM_SYMBOL,...,SEM_TICK_SIZE,...
	for _, row := range records[1:] {
		if len(row) < 16 {
			continue
		}
		segment := row[0] + "_" + row[1]
		securityID := row[2]
		instrumentType := row[3]
		tradingSymbol := row[5]
		customSymbol := row[7]

		lotSize, _ := strconv.ParseFloat(row[6], 64)
		tickSize, _ := strconv.ParseFloat(row[11], 64)

		if _, err := stmt.Exec(securityID, segment, tradingSymbol, customSymbol, instrumentType, int(lotSize), tickSize, today); err != nil {
			return err
		}
	}

	return tx.Commit()
}

func RunScheduler(db *sql.DB) {
	if !alreadySyncedToday(db) {
		log.Println("instruments: syncing now")
		if err := download(db); err != nil {
			log.Printf("instruments: sync failed: %v", err)
		} else {
			log.Println("instruments: sync complete")
		}
	} else {
		log.Println("instruments: already synced today, skipping")
	}

	for {
		d := nextNineAMIST()
		log.Printf("instruments: next sync in %v", d.Round(time.Minute))
		time.Sleep(d)

		log.Println("instruments: syncing")
		if err := download(db); err != nil {
			log.Printf("instruments: sync failed: %v", err)
		} else {
			log.Println("instruments: sync complete")
		}
	}
}
