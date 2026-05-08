alter table strategies add column strategy_type text not null default 'manual';
alter table strategies add column rl_config jsonb;
alter table strategies add column rl_summary jsonb;

create table rl_jobs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  status      text not null default 'pending',
  error       text,
  created_at  timestamptz default now(),
  updated_at  timestamptz default now()
);
