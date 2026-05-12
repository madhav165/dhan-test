-- ============================================================
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

create table nse500_extended (
    symbol         text primary key,
    company_name   text not null,
    industry       text,
    series         text,
    isin           text,
    last_synced    timestamptz default now()
);

