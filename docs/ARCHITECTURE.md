# Architecture

## User mental model

There are four things a user works with:

**Broker connection** — You connect your Dhan account once. The platform stores your access token (encrypted) and uses it on your behalf to fetch market data and place orders. Tokens expire daily; you reconnect each session.

**Strategy** — A piece of logic that looks at a price/volume series and emits signals: hold (0), buy (1), or sell (2). A strategy has no opinion about which instruments it runs on or what dates — it is purely the rule. Strategies are built using the visual rule builder: you define buy and sell conditions using indicators (RSI, SMA, EMA, VWAP, MACD, Bollinger Bands) and raw inputs (price, volume) combined with AND/OR group logic. The platform generates Rust code from the rule tree and compiles it to WebAssembly.

**Run** — A backtest. You pick a strategy, one or more instruments, a date range, and a candle interval. The platform fetches the historical price data, runs the strategy's logic over it, and gives you back performance metrics (trades, PnL, win rate, drawdown) along with the full signal series.

**Policy** — A live deployment. Once a run looks good, you create a policy that activates the strategy on specific instruments in either alert mode (notify you when a signal fires) or trade mode (place the order automatically through Dhan).

The progression is: write strategy → backtest with a run → deploy as a policy.

---

## Internal architecture

### Services

```
Browser  ──▶  SvelteKit (web)  ──▶  Postgres
                    │
                    └──▶  Go service  ──▶  Dhan API
                                │         (historical + live feed)
                                └──▶  Postgres / MinIO
                                │
                                └──▶  Policy worker (on token refresh)

Builder (Rust)   ──▶  Postgres  (polls job queues)
                 └──▶  MinIO    (reads WASM, writes parquet)

Policy worker    ──▶  Postgres  (reads policies/candles, writes live_signals + trade_jobs)
(Rust)           └──▶  Dhan WebSocket feed  (live ticks)
                 └──▶  Telegram Bot API     (alerts)

Trade worker     ──▶  Postgres  (polls trade_jobs, writes trade_positions)
(Rust)           └──▶  Dhan REST API  (margin check, order placement, order status)
                 └──▶  Telegram Bot API  (trade confirmations and rejections)
```

**SvelteKit** handles all UI and server-side data loading. Forms submit directly to `+page.server.ts` actions which write to Postgres. There is no REST API between the frontend and the database — SvelteKit talks to Postgres directly via the `pg` client.

**Go service** does two things: proxies authenticated Dhan API calls from the browser (live market data), and runs the candle worker that fetches and stores historical data before a backtest. It holds the encryption key for broker tokens and is the only service that talks to the Dhan API directly. After storing a refreshed broker token it notifies the policy worker via a local HTTP call.

**Builder** is a Rust binary that runs as a background container. It polls two job queues in Postgres and processes them one at a time:
- `build_jobs` — compiles a strategy's Rust source to WebAssembly and stores the `.wasm` in MinIO
- `run_jobs` — executes a backtest once candle data is ready

**Policy worker** is a Rust binary that executes active policies against live market data. It is triggered by the Go service when a broker token is refreshed, and also runs a 30-second poll loop. For `alert` mode policies it sends Telegram messages directly. For `trade` mode policies it writes to `trade_jobs` and lets the trade worker handle execution.

**Trade worker** is a Rust binary that polls `trade_jobs` and handles order lifecycle: pre-trade validation, order placement, status polling, and position tracking.

### Job queues

All queues are plain Postgres tables polled on a fixed interval. No message broker.

`build_jobs` status flow: `pending → building → done / failed`

`run_jobs` status flow: `pending → ready → running → done / failed`

`trade_jobs` status flow: `pending → checking → placing → polling → done / failed`

The Go candle worker owns the `pending → ready` transition. It checks whether the candles table already has data for the requested instruments, interval, and date range. If not, it calls Dhan's historical API and upserts the rows. Once all instruments are covered it marks the job `ready`.

The builder then picks up `ready` jobs, reads candles from Postgres, loads the compiled WASM from MinIO, and executes it via wasmtime.

### Policy execution

When a user refreshes their broker token the Go service immediately calls `POST /internal/user-connected` on the policy worker. The worker then:

1. Refreshes candles for every instrument in the user's active policies (from the last stored date to today, update-on-conflict)
2. Runs all daily-interval policies immediately — loads close history from Postgres into a fresh WASM instance, calls `run()`, and fires an alert if the signal has changed
3. If the call arrives during market hours (09:15–15:30 IST), spins up intraday runners as well

If the user connects before market opens (e.g. 8 AM), intraday runners are not started. The 30-second poll loop handles this: once 09:15 arrives it checks whether each user with a valid token and active intraday policies already has runners — if not, it starts them.

**Intraday execution** opens one Dhan WebSocket connection per user, subscribed to the union of instruments across all that user's active intraday policies (up to 5000 instruments per connection; Dhan allows 5 concurrent connections per user). For each `(policy, instrument)` pair the worker holds a persistent wasmtime `Store` + `Instance`. On every tick only the last float in WASM memory is updated (the live LTP); the historical close buffer written at startup stays in place. `run()` is called on every tick and a signal transition triggers a write to `live_signals` and a Telegram message.

Intraday runners stop when the WebSocket closes (market close, token expiry, or disconnect). The user's entry is removed from the in-memory active set so the poll loop can restart them if the connection drops and reconnects within market hours.

**Alert delivery** writes a row to `live_signals` (policy, instrument, signal, price, timestamp) and calls the Telegram Bot API with the user's configured `telegram_chat_id`. The policy worker talks to Telegram directly — the Go service is not involved in the signal path.

**Telegram connect** — users link their Telegram account from `/profile/alerts`. The Go service runs a bot poller goroutine (`getUpdates`, 30s long-poll) that listens for `/start` messages. On `/start` the bot generates a 6-digit OTP, stores it in `telegram_link_tokens` with a 10-minute expiry, and sends it to the user. The user pastes the code into the web app; SvelteKit verifies it, saves the `chat_id` to `users.telegram_chat_id`, and sends a confirmation message back via the Bot API. Only `chat_id` is needed for all subsequent alert delivery — no OAuth, no session with Telegram.

**Trade mode** — when a policy's mode is `trade`, the policy worker writes a `trade_jobs` row instead of sending a signal alert, then sends a "trade queued" Telegram message. A deterministic `correlation_id` (hash of policy, instrument, signal direction, and price) is stored on the job to prevent duplicates if the same signal fires on multiple consecutive ticks before the job is processed.

### Trade execution

The trade worker polls `trade_jobs` every 3 seconds and processes each job in sequence:

1. **Value check** — `quantity × price` is compared against `max_trade_value` on `policy_instruments`. If exceeded, the job is rejected and a Telegram alert is sent. This limit applies to both long and short trades.
2. **Margin check** — calls `POST /margincalculator` on the Dhan API with the exact order parameters. If available balance is less than required margin, the job is rejected and a Telegram alert is sent.
3. **Position check** — if an open position already exists for this `(policy, instrument)` in the same direction as the signal, the job is skipped (already in the trade). If the position is in the opposite direction, a closing order is placed first.
4. **Order placement** — calls `POST /orders` with the `correlationId` set to the job's correlation ID (truncated to 30 chars per Dhan's limit). Order type (`MARKET` or `LIMIT`) and product type (`INTRADAY` for intraday-interval policies, `CNC` for daily) are set per policy instrument.
5. **Status polling** — polls `GET /orders/{order-id}` up to 20 times (3 seconds apart) until the order reaches a terminal status: `TRADED`, `PART_TRADED`, `REJECTED`, `CANCELLED`, or `EXPIRED`.
6. **Position recording** — on fill, writes or updates a `trade_positions` row. On failure, marks the job failed and sends a Telegram alert.

`trade_jobs` status flow: `pending → checking → placing → polling → done / failed`

Each instrument in a trade-mode policy carries: `quantity` (integer shares), `order_type` (`MARKET` or `LIMIT`), and `max_trade_value` (0 = no limit). Open positions are tracked in `trade_positions` keyed on `(policy_id, security_id, exchange_segment)`.

### Strategy compilation

When you save a strategy, the source snippet is stored in Postgres. A `build_jobs` row is inserted. The builder wraps your snippet in a scaffold that provides the `alloc` and `run` exports expected by the runtime, adds the `indicators` crate as a dependency, and compiles with `cargo build --target wasm32-unknown-unknown`. The resulting `.wasm` is uploaded to MinIO and the strategy record is updated with its key.

The indicators crate (`rust/indicators`) provides RSI, SMA, EMA, MACD, Bollinger Bands, and VWAP. It is compiled into every strategy WASM — there are no dynamic calls between modules.

The rule tree (buy and sell groups) is stored as JSON in `strategies.rule_json` so the visual builder can re-hydrate it on edit. The generated Rust code is what actually gets compiled — `rule_json` is purely for UI state.

### Backtest execution

The WASM interface is two functions:
- `alloc(len: u32) -> *mut f64` — allocates a price buffer inside WASM memory and returns a pointer
- `run(len: u32) -> *mut u8` — runs the signal logic over the prices and returns a pointer to the signal buffer

The builder writes close prices into WASM memory through `alloc`, calls `run`, and reads back the signal bytes. Signals are 0 (hold), 1 (buy), 2 (sell).

Metrics are computed from the signal series: each buy opens a position at that bar's close, the next sell closes it. PnL, win rate, and max drawdown are calculated across all instruments combined.

Results are written as a Parquet file to MinIO (`runs/{run_id}/result.parquet`) with columns: `security_id`, `exchange_segment`, `timestamp`, `open`, `high`, `low`, `close`, `volume`, `signal`. Summary metrics are written back to the `backtest_runs` row.

### Market data

Historical candles are stored in a shared `candles` table keyed on `(security_id, exchange_segment, interval, timestamp)`. If two runs request the same instrument and date range, the data is fetched once and reused. Intraday data from Dhan has a 90-day per-request limit; the Go worker paginates automatically.

When a chart is opened the Go service checks whether the requested `to` date is today. If so, it fetches from the last stored candle date forward (not just today) so that gaps caused by multi-day absences are filled. The same refresh-from-last-stored-date logic is used by the policy worker before running any policy.

The instrument master (235k rows covering NSE equities, BSE equities, and NSE indices) is synced from Dhan's scrip master CSV at 9 AM IST daily. The UI search queries this table with a prefix-first ordering.

### Storage layout in MinIO

```
strategies/{strategy_id}/source.rs      — full scaffolded Rust source
strategies/{strategy_id}/strategy.wasm  — compiled WebAssembly
runs/{run_id}/result.parquet            — per-candle signals and OHLCV
```

### Encryption

Dhan access tokens are encrypted with AES-256-GCM before storage. The key is a 32-byte value passed as a hex string via `ENCRYPTION_KEY`. The nonce is prepended to the ciphertext and the whole thing is base64-encoded. Only the Go service decrypts tokens.
