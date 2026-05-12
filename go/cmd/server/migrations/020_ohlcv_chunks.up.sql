alter table ohlcv_jobs add column if not exists from_date date;
alter table ohlcv_jobs add column if not exists to_date date;
alter table ohlcv_jobs add column if not exists interval text default '1d';
alter table ohlcv_jobs add column if not exists retry_count int default 0;
alter table ohlcv_jobs add column if not exists max_retries int default 3;

drop index if exists idx_ohlcv_jobs_pending_stock;
create unique index idx_ohlcv_jobs_pending_chunk on ohlcv_jobs (security_id, exchange_segment, from_date, to_date) where status = 'pending';
