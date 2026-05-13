alter table strategies add column if not exists strategy_type text not null default 'manual';
alter table strategies add column if not exists rl_config jsonb;
alter table strategies add column if not exists rl_summary jsonb;

create table if not exists rl_jobs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  status      text not null default 'pending',
  error       text,
  created_at  timestamptz default now(),
  updated_at  timestamptz default now()
);
