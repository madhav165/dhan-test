create table if not exists charts (
  id               uuid primary key default gen_random_uuid(),
  user_id          uuid references users(id) on delete cascade,
  name             text not null,
  security_id      bigint not null,
  exchange_segment text not null,
  interval         text not null,
  indicators       jsonb not null default '[]',
  created_at       timestamptz default now(),
  updated_at       timestamptz default now()
);
create index if not exists idx_charts_user_id on charts (user_id, updated_at desc);
