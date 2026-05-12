package seed

import (
	"database/sql"
	_ "embed"
	"encoding/json"
	"fmt"
	"log"
	"strings"
)

//go:embed nifty500_20220504.json
var json20220504 []byte
//go:embed nifty500_20221009.json
var json20221009 []byte
//go:embed nifty500_20230404.json
var json20230404 []byte
//go:embed nifty500_20240226.json
var json20240226 []byte

type row struct {
	Symbol      string `json:"symbol"`
	CompanyName string `json:"company_name"`
	Industry    string `json:"industry"`
	Series      string `json:"series"`
	ISIN        string `json:"isin"`
}

func Run(db *sql.DB) {
	tables := []struct {
		name    string
		data    []byte
		date    string
	}{
		{"nifty500_20220504", json20220504, "2022-05-04"},
		{"nifty500_20221009", json20221009, "2022-10-09"},
		{"nifty500_20230404", json20230404, "2023-04-04"},
		{"nifty500_20240226", json20240226, "2024-02-26"},
	}

	inserted := 0
	for _, t := range tables {
		var rows []row
		if err := json.Unmarshal(t.data, &rows); err != nil {
			log.Fatalf("parse %s: %v", t.name, err)
		}

		var count int
		err := db.QueryRow("select count(*) from " + t.name).Scan(&count)
		if err != nil {
			log.Printf("check %s: %v (skipping - table may not exist)", t.name, err)
			continue
		}
		if count > 0 {
			log.Printf("%s: %d rows already present, skipping", t.name, count)
			continue
		}

		values := make([]string, len(rows))
		args := make([]interface{}, len(rows)*6)
		for i, r := range rows {
			values[i] = fmt.Sprintf("($%d, $%d, $%d, $%d, $%d, $%d)",
				i*6+1, i*6+2, i*6+3, i*6+4, i*6+5, i*6+6)
			args[i*6] = t.date
			args[i*6+1] = r.Symbol
			args[i*6+2] = r.CompanyName
			args[i*6+3] = r.Industry
			args[i*6+4] = r.Series
			args[i*6+5] = r.ISIN
		}

		sql := fmt.Sprintf(
			"insert into %s (snapshot_date, symbol, company_name, industry, series, isin) values %s",
			t.name, strings.Join(values, ", "),
		)

		_, err = db.Exec(sql, args...)
		if err != nil {
			log.Fatalf("insert into %s: %v", t.name, err)
		}

		inserted += len(rows)
		log.Printf("%s: inserted %d rows", t.name, len(rows))
	}

	log.Printf("seed complete: %d total rows", inserted)

	// Upsert union of all historical tables into nse500_extended
	result, err := db.Exec(`
		insert into nse500_extended (symbol, company_name, industry, series, isin, last_synced)
		select symbol, company_name, industry, series, isin, now()
		from (
			select distinct on (symbol) symbol, company_name, industry, series, isin
			from (
				select symbol, company_name, industry, series, isin, snapshot_date from nifty500_20220504
				union all
				select symbol, company_name, industry, series, isin, snapshot_date from nifty500_20221009
				union all
				select symbol, company_name, industry, series, isin, snapshot_date from nifty500_20230404
				union all
				select symbol, company_name, industry, series, isin, snapshot_date from nifty500_20240226
			) h
			order by symbol, snapshot_date desc
		) latest
		on conflict (symbol) do update set
			company_name = excluded.company_name,
			industry = excluded.industry,
			series = excluded.series,
			isin = excluded.isin,
			last_synced = now()
	`)
	if err != nil {
		log.Fatalf("upsert nse500_extended from historical: %v", err)
	}
	rows, _ := result.RowsAffected()
	log.Printf("nse500_extended: upserted %d rows from historical tables", rows)
}
