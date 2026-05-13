create table if not exists build_jobs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  status      text not null default 'pending',  -- pending / building / done / failed
  error       text,
  created_at  timestamptz default now(),
  updated_at  timestamptz default now()
);

create index if not exists idx_build_jobs_status on build_jobs (status, created_at);
