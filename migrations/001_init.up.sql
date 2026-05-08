create table users (
  id         uuid primary key default gen_random_uuid(),
  email      text unique not null,
  name       text,
  created_at timestamptz default now()
);

create table broker_connections (
  id              uuid primary key default gen_random_uuid(),
  user_id         uuid references users(id) on delete cascade,
  broker          text not null,
  client_id       text not null,
  encrypted_token text,
  token_date      date,
  is_active       boolean default false,
  created_at      timestamptz default now(),
  unique(user_id, broker)
);

create index idx_users_email on users(email);
create index idx_broker_connections_user on broker_connections(user_id);
