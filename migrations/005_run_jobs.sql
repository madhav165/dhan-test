create table run_jobs (
  id          uuid primary key default gen_random_uuid(),
  run_id      uuid references backtest_runs(id) on delete cascade,
  status      text not null default 'pending',  -- pending / running / done / failed
  error       text,
  created_at  timestamptz default now(),
  updated_at  timestamptz default now()
);

create index idx_run_jobs_status on run_jobs (status, created_at);
