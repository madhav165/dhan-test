alter table ohlcv_jobs add column if not exists retry_after timestamptz;
create index if not exists idx_ohlcv_jobs_retry_after on ohlcv_jobs (retry_after) where status = 'pending';
