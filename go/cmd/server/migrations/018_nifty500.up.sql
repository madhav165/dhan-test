create table nifty500_constituents (
  symbol       text primary key,
  company_name text not null,
  industry     text,
  series       text,
  isin         text,
  last_synced  timestamptz default now()
);

create index idx_nifty500_industry on nifty500_constituents(industry);
