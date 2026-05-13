create table if not exists telegram_link_tokens (
    token      text primary key,
    chat_id    text not null,
    created_at timestamptz default now(),
    expires_at timestamptz not null
);
