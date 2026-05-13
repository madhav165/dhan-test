-- per-instrument trading config on policies
alter table policy_instruments
  add column if not exists quantity       int     not null default 1,
  add column if not exists order_type     text    not null default 'MARKET', -- MARKET | LIMIT
  add column if not exists max_trade_value numeric not null default 0;       -- 0 = no limit

-- pending trade requests written by policy worker, consumed by trade worker
create table if not exists trade_jobs (
  id             uuid primary key default gen_random_uuid(),
  policy_id      uuid references policies(id) on delete cascade,
  security_id    text        not null,
  exchange_segment text      not null,
  signal         text        not null,  -- BUY | SELL
  price          numeric     not null,
  quantity       int         not null,
  order_type     text        not null,
  product_type   text        not null,  -- INTRADAY | CNC
  correlation_id text        not null unique,
  status         text        not null default 'pending', -- pending | checking | placing | polling | done | failed
  order_id       text,
  order_status   text,
  error          text,
  created_at     timestamptz default now(),
  updated_at     timestamptz default now()
);

create index if not exists idx_trade_jobs_status on trade_jobs (status, created_at);

-- open positions per (policy, instrument)
create table if not exists trade_positions (
  policy_id        uuid references policies(id) on delete cascade,
  security_id      text    not null,
  exchange_segment text    not null,
  direction        text    not null,  -- LONG | SHORT
  quantity         int     not null,
  entry_price      numeric not null,
  opened_at        timestamptz default now(),
  primary key (policy_id, security_id, exchange_segment)
);
