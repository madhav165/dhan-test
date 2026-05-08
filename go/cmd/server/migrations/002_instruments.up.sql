create table instruments (
  security_id      text not null,
  exchange_segment text not null,
  trading_symbol   text not null,
  custom_symbol    text,
  isin             text,
  instrument_type  text,
  lot_size         int,
  tick_size        numeric,
  last_updated     date not null,
  primary key (security_id, exchange_segment)
);

create index idx_instruments_symbol on instruments(trading_symbol);
create index idx_instruments_custom_symbol on instruments(custom_symbol);
create index idx_instruments_segment on instruments(exchange_segment);
