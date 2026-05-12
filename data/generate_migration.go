package main

import (
	"bufio"
	"encoding/csv"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	csvDir := "data/nse500"
	migrationFile := "go/cmd/server/migrations/022_nse500_historical.up.sql"

	migrationDir := filepath.Dir(migrationFile)
	if err := os.MkdirAll(migrationDir, 0755); err != nil {
		panic(err)
	}

	f, err := os.Create(migrationFile)
	if err != nil {
		panic(err)
	}
	defer f.Close()

	writer := bufio.NewWriter(f)

	// Header
	fmt.Fprint(writer, `-- ============================================================
-- 022_nse500_historical.up.sql
-- Auto-generated from CSV files in data/nse500/
-- DO NOT EDIT - regenerate with go run data/generate_migration.go
-- ============================================================

create table nifty500_20220504 (
    snapshot_date  date not null,
    symbol         text not null,
    company_name   text not null,
    industry       text,
    series         text,
    isin           text
);

create table nifty500_20221009 (
    snapshot_date  date not null,
    symbol         text not null,
    company_name   text not null,
    industry       text,
    series         text,
    isin           text
);

create table nifty500_20230404 (
    snapshot_date  date not null,
    symbol         text not null,
    company_name   text not null,
    industry       text,
    series         text,
    isin           text
);

create table nifty500_20240226 (
    snapshot_date  date not null,
    symbol         text not null,
    company_name   text not null,
    industry       text,
    series         text,
    isin           text
);

create index idx_nifty500_20220504_symbol on nifty500_20220504(symbol);
create index idx_nifty500_20221009_symbol on nifty500_20221009(symbol);
create index idx_nifty500_20230404_symbol on nifty500_20230404(symbol);
create index idx_nifty500_20240226_symbol on nifty500_20240226(symbol);

`)

	csvs := []struct {
		table   string
		csvName string
	}{
		{"nifty500_20220504", "nse500_20220504.csv"},
		{"nifty500_20221009", "nse500_20221009.csv"},
		{"nifty500_20230404", "nse500_20230404.csv"},
		{"nifty500_20240226", "nse500_20240226.csv"},
	}

	for _, c := range csvs {
		csvPath := filepath.Join(csvDir, c.csvName)
		rows, err := readCSV(csvPath)
		if err != nil {
			panic(err)
		}

		fmt.Fprintf(writer, "\n-- Seed %s (%d rows)\n", c.table, len(rows))
		fmt.Fprintf(writer, "insert into %s (snapshot_date, symbol, company_name, industry, series, isin)\n", c.table)
		fmt.Fprintf(writer, "select * from json_populate_recordset(null::%s, $1::text)\n", c.table)
		fmt.Fprintf(writer, "where not exists (select 1 from %s limit 1);\n\n", c.table)

		// Also write the JSON data as a separate file for runtime seeding
		jsonFile := filepath.Join(csvDir, c.table+".json")
		writeJSON(jsonFile, rows)
	}

	writer.Flush()
	fmt.Printf("Generated %s (%d rows total)\n", migrationFile, countTotalRows(csvs))
}

type row struct {
	companyName string
	industry    string
	symbol      string
	series      string
	isin        string
}

func readCSV(path string) ([]row, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	reader := csv.NewReader(f)
	_, err = reader.Read() // skip header
	if err != nil {
		return nil, err
	}

	var rows []row
	for {
		record, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, err
		}
		// CSV order: Company Name, Industry, Symbol, Series, ISIN Code
		rows = append(rows, row{
			companyName: record[0],
			industry:    record[1],
			symbol:      record[2],
			series:      record[3],
			isin:        record[4],
		})
	}
	return rows, nil
}

func writeJSON(path string, rows []row) {
	var sb strings.Builder
	sb.WriteString("[\n")
	for i, r := range rows {
		if i > 0 {
			sb.WriteString(",\n")
		}
		sb.WriteString(fmt.Sprintf(
			`  {"symbol":%q,"company_name":%q,"industry":%q,"series":%q,"isin":%q}`,
			r.symbol, r.companyName, r.industry, r.series, r.isin,
		))
	}
	sb.WriteString("\n]")
	os.WriteFile(path, []byte(sb.String()), 0644)
}

func countTotalRows(csvs []struct{ table string; csvName string }) int {
	total := 0
	for _, c := range csvs {
		rows, _ := readCSV(filepath.Join("data/nse500", c.csvName))
		total += len(rows)
	}
	return total
}
