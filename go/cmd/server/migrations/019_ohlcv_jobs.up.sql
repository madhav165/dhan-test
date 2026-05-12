create table ohlcv_jobs (
  id               uuid primary key default gen_random_uuid(),
  security_id      text not null,
  exchange_segment text not null,
  status           text not null default 'pending',
  error            text,
  created_at       timestamptz default now(),
  updated_at       timestamptz default now()
);

create unique index idx_ohlcv_jobs_pending_stock on ohlcv_jobs (security_id, exchange_segment) where status = 'pending';
create index idx_ohlcv_jobs_status on ohlcv_jobs (status, created_at);
