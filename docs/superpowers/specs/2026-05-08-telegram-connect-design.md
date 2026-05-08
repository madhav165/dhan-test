# Telegram Connect Design

## Overview

Allow users to link their Telegram account to receive policy alerts. Verification uses a bot-generated OTP so the app can confirm the user owns the `chat_id` before saving it.

## Database

New table:

```sql
create table telegram_link_tokens (
    token      text primary key,
    chat_id    text not null,
    created_at timestamptz default now(),
    expires_at timestamptz not null
);
```

Existing column `users.telegram_chat_id text` (migration 010) stores the verified chat_id. No additional column needed.

## Bot Poller (Go server)

A goroutine started at server boot polls `getUpdates` every 2 seconds. On receiving a `/start` message:

1. Generate a random 6-digit numeric OTP
2. Insert `(otp, chat_id, now() + 10min)` into `telegram_link_tokens`
3. Call `sendMessage` to that `chat_id`: "Your connection code is: **XXXXXX** (expires in 10 minutes)"

Runs inside the existing Go server process — no separate service.

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/telegram/status` | session | Returns `{ connected: bool }` |
| POST | `/api/telegram/verify` | session | Body `{ token }` → validates OTP, saves `chat_id`, deletes token |
| POST | `/api/telegram/disconnect` | session | Clears `users.telegram_chat_id` |

`POST /api/telegram/verify` returns 400 if the token is missing, expired, or not found. On success it sends a confirmation message to the user via the bot: "✅ Connected to Dhan."

## UI — `/profile/alerts`

**Disconnected state:**
- Explainer: "Get signal and trade alerts on Telegram"
- QR code image + "Open in Telegram" link → `t.me/BOTNAME`
- Instruction: "Send /start to the bot, then enter the code below"
- Text input + Connect button
- Inline error on invalid/expired code

**Connected state:**
- "✅ Telegram connected"
- Disconnect button (calls `/api/telegram/disconnect`, returns to disconnected state)

## Environment Variables

`TELEGRAM_BOT_TOKEN` — already present in `AppState`. Add to `.env.example`.
`TELEGRAM_BOT_NAME` — used to construct the `t.me/` link and QR code. Add to `.env.example`.

## Error Handling

- Invalid or expired OTP → 400, UI shows inline error
- Bot poll failure → logged, retried next tick
- Disconnect when already disconnected → 200 no-op
