create table if not exists candles (
  security_id      text        not null,
  exchange_segment text        not null,
  interval         text        not null,
  timestamp        timestamptz not null,
  open             numeric     not null,
  high             numeric     not null,
  low              numeric     not null,
  close            numeric     not null,
  volume           bigint      not null,
  primary key (security_id, exchange_segment, interval, timestamp)
);

create index if not exists idx_candles_lookup on candles (security_id, exchange_segment, interval, timestamp desc);

create table if not exists strategies (
  id         uuid primary key default gen_random_uuid(),
  user_id    uuid references users(id) on delete cascade,
  name       text not null,
  source_key text,
  wasm_key   text,
  created_at timestamptz default now()
);

create index if not exists idx_strategies_user on strategies (user_id);

create table if not exists backtest_runs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  interval    text not null,
  from_date   date not null,
  to_date     date not null,
  run_at      timestamptz default now(),
  result_key  text,
  num_trades  int,
  total_pnl   numeric,
  win_rate    numeric,
  max_drawdown numeric
);

create index if not exists idx_backtest_runs_strategy on backtest_runs (strategy_id);

create table if not exists backtest_run_instruments (
  run_id           uuid references backtest_runs(id) on delete cascade,
  security_id      text not null,
  exchange_segment text not null,
  primary key (run_id, security_id, exchange_segment)
);

create table if not exists policies (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  mode        text not null,  -- alert / trade
  status      text not null default 'active',  -- active / paused
  created_at  timestamptz default now()
);

create index if not exists idx_policies_strategy on policies (strategy_id);

create table if not exists policy_instruments (
  policy_id        uuid references policies(id) on delete cascade,
  security_id      text not null,
  exchange_segment text not null,
  primary key (policy_id, security_id, exchange_segment)
);

create table if not exists live_signals (
  id           uuid primary key default gen_random_uuid(),
  policy_id    uuid references policies(id) on delete cascade,
  security_id  text        not null,
  triggered_at timestamptz not null,
  signal       text        not null,
  price        numeric     not null
);

create index if not exists idx_live_signals_policy on live_signals (policy_id, triggered_at desc);
