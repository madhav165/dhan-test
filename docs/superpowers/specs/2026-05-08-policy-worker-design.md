# Policy Worker Design

**Date:** 2026-05-08

## Overview

A new `policy-worker` Rust binary executes active policies against live market data. It runs alongside the existing `builder` binary. When a policy's signal transitions, the worker writes to `live_signals` in DB and sends a Telegram alert.

Policies have two modes (only `alert` is in scope now; `trade` comes later) and two interval classes: daily and intraday.

---

## Trigger: User Connects

The Go server's `POST /internal/broker-token` endpoint, after storing the token, immediately calls `POST /internal/user-connected?user_id=...` on the policy worker (local HTTP). This is the single activation event for both daily and intraday policies. Zero delay — users expect to see signal activity as soon as they connect their broker.

---

## Daily Policies

**When:** immediately on `user-connected`, regardless of time of day.

**Sequence for each active daily policy the user owns:**

1. For each instrument in the policy, refresh candles from Dhan API (`FetchAndStore` from `max(stored date)` to today, `update=true`) and upsert into DB
2. Load full close history from DB into a fresh WASM buffer
3. Call `run()`, compare result to `last_signal` stored in memory
4. On transition: insert row into `live_signals`, send Telegram message
5. Store new `last_signal`

---

## Intraday Policies

**When:** only during market hours (09:15–15:30 IST). If `user-connected` fires outside this window, intraday runners are not created. The 30s DB poll also skips intraday runner creation outside market hours.

**Sequence on `user-connected` during market hours:**

1. For each instrument across all the user's active intraday policies, refresh today's candles from Dhan API (from `max(stored date)` to today, `update=true`) and upsert into DB
2. Load historical closes from DB into a WASM buffer per `(policy, instrument)`. Allocate `n+1` slots — first `n` are closed candles, last slot reserved for live LTP
3. Open one Dhan WebSocket connection for the user, subscribe to the union of all instruments (up to 5000 per connection; one connection per user is sufficient)
4. On each tick for an instrument:
   - Write LTP into the last slot of every `(policy, instrument)` buffer that watches this instrument
   - Call WASM `run()` for each such policy
   - On signal transition: insert `live_signals` row, send Telegram alert
   - Store new `last_signal`

**WASM instance lifetime:** one persistent `wasmtime Store + Instance` per `(policy, instrument)`. `alloc` is called once at startup; only the last float in WASM memory is updated per tick. This avoids re-instantiation overhead on every tick.

---

## Policy State Management

A 30s DB poll loop reloads all active policies. On change:

- New policy added → refresh candles, create WASM runner, subscribe instrument if not already subscribed
- Policy paused/deleted → drop WASM runner, unsubscribe instrument from WebSocket if no other active policy for this user needs it
- Intraday runners: poll skips creation entirely outside market hours

---

## Alert Delivery

On signal transition (`last_signal != new_signal` and `new_signal != 0`):

1. `INSERT INTO live_signals (policy_id, security_id, triggered_at, signal, price)`
2. Call Telegram Bot API to send message to the user's configured chat

Telegram config (bot token, chat ID per user) stored in DB — a new `telegram_config` table or column on `users`.

Signal values: `1` = long, `2` = short (matching existing WASM contract).

---

## Infrastructure

The policy worker is a new Rust binary at `policy-worker/src/main.rs`. It shares the same environment variables as `builder` (`DATABASE_URL`, `MINIO_*`, `DHAN_BASE_URL`) plus new ones (`TELEGRAM_BOT_TOKEN`, `POLICY_WORKER_PORT`).

Go server gains one new outbound call: after `POST /internal/broker-token` succeeds, it fires `POST http://localhost:{POLICY_WORKER_PORT}/internal/user-connected?user_id=...`. Non-fatal if the policy worker is down.

Dhan WebSocket limits: 5 connections per user, 5000 instruments per connection. One connection per user is sufficient.

---

## Out of Scope

- `trade` mode (automated order placement via Dhan order API)
- Multi-user WebSocket fan-out (not needed on developer plan)
- Push notifications beyond Telegram (email, SMS, in-app)
