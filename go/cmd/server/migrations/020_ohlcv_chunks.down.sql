drop index if exists idx_ohlcv_jobs_pending_chunk;
create unique index idx_ohlcv_jobs_pending_stock on ohlcv_jobs (security_id, exchange_segment) where status = 'pending';

alter table ohlcv_jobs drop column if exists from_date;
alter table ohlcv_jobs drop column if exists to_date;
alter table ohlcv_jobs drop column if exists interval;
alter table ohlcv_jobs drop column if exists retry_count;
alter table ohlcv_jobs drop column if exists max_retries;
