# Architecture

## User mental model

There are four things a user works with:

**Broker connection** — You connect your Dhan account once. The platform stores your access token (encrypted) and uses it on your behalf to fetch market data and place orders. Tokens expire daily; you reconnect each session.

**Strategy** — A piece of logic that looks at a price/volume series and emits signals: hold (0), buy (1), or sell (2). A strategy has no opinion about which instruments it runs on or what dates — it is purely the rule. Strategies are created either through the visual rule builder — defining buy and sell conditions using indicators (RSI, SMA, EMA, WMA, VWAP, MACD, Bollinger Bands, ATR, Stochastic, OBV, CCI) and raw inputs (price, volume) combined with AND/OR group logic — or through reinforcement learning (PPO/REINFORCE), where the platform trains an Actor-Critic neural network on historical data and distills it into an executable strategy. The platform generates Rust code and compiles it to WebAssembly.

**Run** — A backtest. You pick a strategy, one or more instruments, a date range, and a candle interval. The platform fetches the historical price data, runs the strategy's logic over it, and gives you back performance metrics (trades, PnL, win rate, drawdown) along with the full signal series.

**Policy** — A live deployment. Once a run looks good, you create a policy that activates the strategy on specific instruments in either alert mode (notify you when a signal fires) or trade mode (place the order automatically through Dhan).

The progression is: write strategy → backtest with a run → deploy as a policy.

---

## Authentication

The platform uses two independent OAuth flows:

**Google OAuth** — Authenticates the user into the application. The SvelteKit frontend initiates a PKCE flow via `GET /auth/google`, stores the state and code verifier in httpOnly cookies, and redirects to Google. The callback at `GET /auth/google/callback` validates the ID token claims (openid, email, profile scopes), upserts the user into the `users` table, and issues a JWT session cookie (HS256, 30-day expiry). The session middleware in `hooks.server.ts` verifies the JWT on every request and populates `event.locals.user`.

**Dhan OAuth** — Connects the user's broker account for trading and market data. Initiated via `GET /auth/dhan`, which calls `POST /app/generate-consent` on the Dhan API to obtain a `consentAppId`, then redirects the user to Dhan's consent login. The callback at `GET /auth/dhan/callback` exchanges the returned `tokenId` for `dhanClientId` + `accessToken` via `POST /app/consumeApp-consent`. The token pair is sent to the Go service at `POST /internal/broker-token`, where it is encrypted with AES-256-GCM and stored in the `broker_connections` table. The Go service is the only component that decrypts tokens.

Tokens expire daily; the user must reconnect each session. After a token refresh the Go service notifies the policy worker via `POST /internal/user-connected`.

---

## Internal architecture

### Services

```
Browser  ──▶  SvelteKit (web)  ──▶  Postgres
                    │
                    ├──▶  Go service  ──▶  Dhan API
                    │               │         (historical + live market data, LTP, OHLC, quotes)
                    │               └──▶  Postgres / MinIO
                    │               │
                    │               ├──▶  Policy worker (on token refresh)
                    │               │
                    │               └──▶  NSE India API  (Nifty500 constituent CSV)
                    │
                    └──▶  Google OAuth  (user authentication)

Builder (Rust)   ──▶  Postgres  (polls build_jobs, run_jobs, rl_jobs)
                 └──▶  MinIO    (reads WASM, writes parquet + signals.json + weights)

Policy worker    ──▶  Postgres  (reads policies/candles, writes live_signals + trade_jobs)
(Rust)           └──▶  Dhan WebSocket feed  (live ticks)
                 └──▶  Telegram Bot API     (alerts)

Trade worker     ──▶  Postgres  (polls trade_jobs, writes trade_positions)
(Rust)           └──▶  Dhan REST API  (margin check, order placement, order status)
                 └──▶  Telegram Bot API  (trade confirmations and rejections)
```

**SvelteKit** handles all UI and server-side data loading. Forms submit directly to `+page.server.ts` actions which write to Postgres. There is no REST API between the frontend and the database — SvelteKit talks to Postgres directly via the `pg` client.

**Go service** does two things: proxies authenticated Dhan API calls from the browser (live market data), and runs the candle worker that fetches and stores historical data before a backtest. It holds the encryption key for broker tokens and is the only service that talks to the Dhan API directly. After storing a refreshed broker token it notifies the policy worker via a local HTTP call.

**Builder** is a Rust binary that runs as a background container. It polls three job queues in Postgres and processes them one at a time:
- `build_jobs` — compiles a strategy's Rust source to WebAssembly and stores the `.wasm` in MinIO
- `run_jobs` — executes a backtest once candle data is ready
- `rl_jobs` — runs RL strategy training: feature engineering, PPO/REINFORCE training, model distillation, code generation, and WASM compilation

**Policy worker** is a Rust binary that executes active policies against live market data. It is triggered by the Go service when a broker token is refreshed, and also runs a 30-second poll loop. For `alert` mode policies it sends Telegram messages directly. For `trade` mode policies it writes to `trade_jobs` and lets the trade worker handle execution.

**Trade worker** is a Rust binary that polls `trade_jobs` and handles order lifecycle: pre-trade validation, order placement, status polling, and position tracking.

### Job queues

All queues are plain Postgres tables polled on a fixed interval. No message broker.

`build_jobs` status flow: `pending → building → done / failed`

`run_jobs` status flow: `pending → ready → running → done / failed`

`trade_jobs` status flow: `pending → checking → placing → polling → done / failed`

`rl_jobs` status flow: `pending → training → running → done / failed`

The Go run worker handles the `pending → training` transition for `rl_jobs`. It claims a pending job, ensures all required candle data exists (fetching via Dhan API if needed), then marks the job as `training`. The builder picks up `training` jobs, runs the full RL pipeline (feature engineering, training, distillation, code generation, WASM compilation), stores artifacts in MinIO, and marks the job `done`.

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

The indicators crate (`rust/indicators`) provides RSI, SMA, EMA, WMA, MACD, Bollinger Bands, VWAP, ATR, Stochastic, OBV, and CCI. It is compiled into every strategy WASM — there are no dynamic calls between modules. A separate `indicators-wasm` crate exposes the same functions as `extern "C"` exports for browser-side chart rendering via TypeScript bindings.

The rule tree (buy and sell groups) is stored as JSON in `strategies.rule_json` so the visual builder can re-hydrate it on edit. The generated Rust code is what actually gets compiled — `rule_json` is purely for UI state.

### RL strategy training

In addition to the visual rule builder, users can create strategies via reinforcement learning. The user configures training parameters (algorithm, reward function, constraints, lookback, indicators) through the UI, which creates a strategy record and inserts an `rl_jobs` row.

**Training pipeline** — When the builder picks up an `rl_jobs` entry, it runs the full pipeline:

1. **Feature engineering** — Computes indicators (RSI, SMA, EMA, WMA, MACD, BB, VWAP, ATR, Stochastic, OBV, CCI) over the historical OHLCV data, then applies stationary transforms (EMA distance, RSI centering, BB bandwidth/%B, OBV delta normalization, velocity features). The state matrix combines indicator values with OHLCV log returns, normalized via z-score.

2. **Training** — Trains an Actor-Critic MLP using either PPO or REINFORCE. The Actor network outputs discrete actions (hold/buy/sell via softmax) or continuous actions (Gaussian with tanh-mean). The Critic network is a single-output MLP with MSE loss. Both networks use configurable hidden size, layer count, and activation (Tanh/ReLU) with He initialization, Adam optimizer, gradient clipping, and L1/L2 regularization. PPO uses clipped ratio with entropy bonus and GAE for advantage estimation; rollouts are parallelized via rayon.

3. **Distillation** — After training, the model is distilled into a compact decision tree. Feature importance is computed via permutation-based scoring. The trained MLP weights can also be directly converted to Rust code via `net_to_rust`, which generates baked-in weight arrays for the compiled strategy.

4. **Compilation** — The distilled/generated Rust code is scaffolded and compiled to WASM, uploaded to MinIO, and the strategy record is updated.

**RL config** — Each RL strategy stores its training configuration (`rl_config`) and post-training summary (`rl_summary`) as JSON in the `strategies` table. Per-episode training metrics are stored in `rl_training_metrics`.

### Backtest execution

The WASM interface is two functions:
- `alloc(len: u32) -> *mut f64` — allocates a price buffer inside WASM memory and returns a pointer
- `run(len: u32) -> *mut u8` — runs the signal logic over the prices and returns a pointer to the signal buffer

The builder writes close prices into WASM memory through `alloc`, calls `run`, and reads back the signal bytes. Signals are 0 (hold), 1 (buy), 2 (sell).

Metrics are computed from the signal series: each buy opens a position at that bar's close, the next sell closes it. PnL, win rate, and max drawdown are calculated across all instruments combined.

Results are written as a Parquet file to MinIO (`runs/{run_id}/result.parquet`) with columns: `security_id`, `exchange_segment`, `timestamp`, `open`, `high`, `low`, `close`, `volume`, `signal`. Summary metrics are written back to the `backtest_runs` row.

### Market data

**Historical candles** are stored in a shared `candles` table keyed on `(security_id, exchange_segment, interval, timestamp)`. If two runs request the same instrument and date range, the data is fetched once and reused. Intraday data from Dhan has a 90-day per-request limit; the Go worker paginates automatically.

When a chart is opened the Go service checks whether the requested `to` date is today. If so, it fetches from the last stored candle date forward (not just today) so that gaps caused by multi-day absences are filled. The same refresh-from-last-stored-date logic is used by the policy worker before running any policy.

The instrument master (235k rows covering NSE equities, BSE equities, and NSE indices) is synced from Dhan's scrip master CSV at 9 AM IST daily. The UI search queries this table with a prefix-first ordering.

**Nifty500 constituents** — The Go service maintains a `nifty500_constituents` table by downloading the official CSV from NSE India (`https://archives.nseindia.com/content/indices/ind_nifty500list.csv`). A scheduler runs an initial sync at boot, then schedules a refresh for the 1st of every month at 9:00 AM IST. Before each sync, old data is archived into `nifty500_snapshots` for historical tracking. Duplicate detection compares against the latest snapshot to skip no-op syncs. An extended view (`nse500_extended`) joins constituents with the instrument master. A `GET /nifty500` API endpoint exposes the current list.

**OHLCV background worker** — A dedicated Go worker (`go/internal/ohlcv/worker.go`) maintains up-to-date daily candle data for all NSE500 stocks. On boot it creates jobs for all NSE500 instruments in the `ohlcv_jobs` table, then schedules daily execution at 4:00 PM IST. The worker runs 5 concurrent goroutines with a 232ms stagger, claiming jobs via `FOR UPDATE SKIP LOCKED` for concurrent-safe processing. Each job fetches daily candles from Dhan's historical API in 90-day chunks, with per-user rate limiting via a token bucket. Failed jobs retry up to 3 times with exponential backoff; 400 errors are permanently failed.

**Market data API proxies** — The Go service proxies authenticated Dhan market data calls from the browser. All three endpoints (`POST /market/ltp`, `POST /market/ohlc`, `POST /market/quote`) inject the user's Dhan access token and client ID, then reverse-proxy to the corresponding Dhan API endpoint. Rate limiting is enforced per user: 1 request/second with a maximum of 5 concurrent requests.

**Live chart WebSocket feed** — A WebSocket endpoint at `GET /chart/live` upgrades the browser connection and proxies Dhan's real-time binary feed. The Go service opens a WebSocket to `wss://api-feed.dhan.co`, subscribes to the requested instrument (RequestCode 17), and parses 50-byte binary packets (type 4) to extract LTP (float32), LTT (uint32), and Volume (uint32) from little-endian byte offsets. The parsed data is forwarded to the browser as JSON for live charting. WebSocket connections are authenticated via HMAC-SHA256 signed tokens.

### Saved charts

Users can save chart configurations through the `/charts` page. Each saved chart stores the instrument (`security_id`, `exchange_segment`), candle interval, and a JSON array of indicator configurations (type, parameters, visual settings). The save action upserts into the `charts` table, scoped to the user. Saved charts can be loaded, edited, or deleted. When loading a chart, the indicator configurations are re-hydrated so the chart page renders with the exact same indicators and settings.

### Run management

**CRUD operations** — The `/runs` page lists all backtest runs for the user, showing strategy name, job status, symbols, and PnL metrics. The `/runs/new` page creates a new run by inserting into `backtest_runs` and `backtest_run_instruments`, plus a corresponding `run_jobs` row. The `/runs/[run_id]` page displays run details including instruments, job status, and any error messages. A delete action removes the run's MinIO artifacts via the Go API (`DELETE /result/run/{run_id}`), then deletes the database row.

**Strategy source retrieval** — The Go service provides `GET /result/strategy-source?run_id=...` which fetches the original scaffolded Rust source from MinIO and extracts the user's signal function snippet between `fn signal{...}` delimiters, useful for auditing and debugging backtest results.

### Storage layout in MinIO

```
strategies/{strategy_id}/source.rs      — full scaffolded Rust source
strategies/{strategy_id}/strategy.wasm  — compiled WebAssembly
runs/{run_id}/result.parquet            — per-candle signals and OHLCV
runs/{run_id}/signals.json              — signal series per instrument
runs/{run_id}/weights.bin               — RL trained model weights (if applicable)
```

### WASM position management

The strategy compilation scaffold and backtest engine implement advanced position management logic inside the WASM runtime:

**Deadband filter** — A configurable `POSITION_DEADBAND` (default 0.05) prevents excessive signal flipping. A new signal is only acted upon when the strategy's conviction (absolute output value) differs from the current position by more than the deadband threshold. This applies during both backtest execution and live policy evaluation.

**Continuous position sizing** — The WASM scaffold supports continuous position signals, where the strategy output is a float representing desired position size. The runtime tracks `current_position` as a float and computes `size_delta` on each bar, allowing fractional position adjustments rather than binary buy/sell decisions.

**EOD force-close** — During backtest execution, the engine detects end-of-day boundaries by comparing `timestamps[i]/86400` against `timestamps[i+1]/86400`. When a day boundary is detected for intraday-interval strategies, any open position is force-closed at the bar's close price. The policy worker mirrors this behavior for live execution: at market close, it queries `trade_positions` for all open intraday positions and queues MARKET close orders with a `-eod-close` correlation ID suffix.

**OHLCV allocation** — The backtest engine computes a warmup buffer based on lookback period, max indicator period, and velocity lookback, fetching extra candles before the configured `from_date` to ensure indicators are fully primed for the first bar in the range.

### Admin features

The admin panel at `/admin` is restricted to a specific user ID (`OHLCV_USER_ID`). It provides:

**OHLCV data management** — Displays real-time statistics for the OHLCV background worker (total jobs, completed, in-progress, failed counts). An admin can trigger a manual OHLCV refresh via `POST /admin/ohlcv/trigger`, which calls the Go backend at `POST /internal/ohlcv-trigger`. A WebSocket endpoint at `GET /admin/ohlcv/ws` provides live progress updates, authenticated via HMAC-signed tokens.

**Stock list** — `GET /ohlcv/stocks` returns a paginated list of stocks with their OHLCV coverage status, allowing admins to monitor data completeness across the NSE500 universe.

### Encryption

Dhan access tokens are encrypted with AES-256-GCM before storage. The key is a 32-byte value passed as a hex string via `ENCRYPTION_KEY`. The nonce is prepended to the ciphertext and the whole thing is base64-encoded. Only the Go service decrypts tokens.
