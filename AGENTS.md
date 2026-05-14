This document is the single source of truth for any developer or agent working on this codebase. It combines **system patterns** (how things work) with **concrete file paths** (where to find them). Organized as a tree of increasingly bigger concepts — from foundational principles to high-level system patterns, with code locations woven throughout.

---

## QUICK REFERENCE: FILES BY FEATURE

### Authentication & Sessions
| Feature | Files |
|---------|-------|
| Google OAuth | `web/src/routes/auth/google/+server.ts:6` (init), `web/src/routes/auth/google/callback/+server.ts:8` (callback), `web/src/lib/server/oauth.ts:4` (arctic provider) |
| Dhan OAuth | `web/src/routes/auth/dhan/+server.ts:6` (init, lines 6-33), `web/src/routes/auth/dhan/callback/+server.ts:5` (callback, lines 5-39) |
| Token Encryption | `go/internal/broker/handler.go:27` (AES-256-GCM), `go/internal/broker/token.go:9` (GetToken decrypts) |
| JWT Session | `web/src/lib/server/session.ts:1` (HS256, 30d, jose), `web/src/hooks.server.ts:5` (verifies on every request) |
| Auth Store | `web/src/lib/stores/auth.ts:1` (Svelte writable: user, brokerConnected) |

### Market Data
| Feature | Files |
|---------|-------|
| Nifty500 Sync | `go/internal/nifty500/sync.go` (lines 1-195, RunScheduler at :171) |
| Nifty500 API | `go/internal/nifty500/handler.go:21` (GET /nifty500) |
| OHLCV Worker | `go/internal/ohlcv/worker.go` (lines 1-437, Start at :31, next4PMIST at :60) |
| OHLCV Admin | `go/internal/ohlcv/handler.go:15` (stats), :348 (WebSocket), :189 (routes) |
| Candle Fetching | `go/internal/candles/fetch.go:56` (FetchAndStore), :147 (FetchChunk), :204 (Upsert) |
| Market Proxies | `go/internal/market/handler.go:32` (proxyDhan), :87-89 (LTP/OHLC/Quote routes) |
| Live WebSocket | `go/internal/live/handler.go:24` (Dhan binary feed), :86 (parseQuote), :45 (MakeToken HMAC), :170 (GET /chart/live) |

### Strategy & Indicators
| Feature | Files |
|---------|-------|
| Rust Indicators | `rust/indicators/src/` — wma.rs:1, atr.rs:4, stoch.rs:4, obv.rs:4, cci.rs:4, rsi.rs, sma.rs, ema.rs, macd.rs, bb.rs, vwap.rs |
| WASM Exports | `rust/indicators-wasm/src/lib.rs:1` (extern "C" functions) |
| TS Bindings | `web/src/lib/wasm/indicators.ts:1` (loadIndicators, runIndicator, etc.) |
| RL Config Types | `web/src/lib/types/rl.ts:1` (RLConfig, RLSummary, etc.) |
| RL Strategy UI | `web/src/routes/strategies/new/rl/+page.server.ts:1` (creates strategy + rl_jobs) |

### RL Training
| Feature | Files |
|---------|-------|
| MLP Network | `builder/src/rl/train.rs:14` (lines 14-163, Adam optimizer at :99) |
| Actor (Policy) | `builder/src/rl/train.rs:166` (lines 166-395, REINFORCE at :234, PPO at :298) |
| Critic | `builder/src/rl/train.rs:397` (lines 397-444, MSE loss) |
| train_reinforce | `builder/src/rl/train.rs:1206` |
| train_ppo | `builder/src/rl/train.rs:1323` (rayon parallel, GAE, clipped ratio) |
| Distillation | `builder/src/rl/distill.rs:283` (net_to_rust), :367 (distil), :159 (feature_importance) |
| Feature Engineering | `builder/src/rl/features.rs:30` (IndicatorSpec), :63 (compute_indicators), :116 (stationary_transform), :230 (build_state_matrix), :285 (normalise_with_stats) |

### Execution & WASM Runtime
| Feature | Files |
|---------|-------|
| Builder Main | `builder/src/main.rs` — scaffold at :107 (POSITION_DEADBAND), :704 (build_parquet + signals.json), :847 (process_rl_job), :1213 (main loop), :1228 (build_jobs), :1279 (run_jobs), :1309 (rl_jobs) |
| Policy Worker | `policy-worker/src/main.rs` — WasmRunner at :45, is_market_hours at :561, run_daily at :588, run_intraday at :635, record_signal at :477, EOD close at :761 |
| Trade Worker | `trade-worker/src/main.rs:54` (check_funds), :94 (check_margin), :248 (max_trade_value), lines 1-460 total |
| Run Worker (Go) | `go/internal/run/worker.go:23` (Start), :31 (pollRuns), :56 (pollRLJobs), :110 (candle fetch) |

### Backtest Runs & Results
| Feature | Files |
|---------|-------|
| List Runs | `web/src/routes/runs/+page.server.ts:5` (CRUD actions at :28 save, :40 delete) |
| Create Run | `web/src/routes/runs/new/+page.server.ts:21` |
| View Run | `web/src/routes/runs/[run_id]/+page.server.ts:5` |
| Strategy Source | `go/internal/result/handler.go:129` (StrategySource extracts snippet) |
| Run Result | `go/internal/result/handler.go:41` (RunResult fetches signals.json from MinIO) |
| Delete Run | `go/internal/result/handler.go:94` (removes parquet + signals.json) |

### UI/UX
| Feature | Files |
|---------|-------|
| Saved Charts | `web/src/routes/charts/+page.server.ts:1` (load at :15, save at :28, delete at :65, makeWsToken at :7) |
| Admin Panel | `web/src/routes/admin/+page.server.ts:1` (OHLCV_USER_ID check at :14) |
| Admin OHLCV Trigger | `web/src/routes/admin/ohlcv/trigger/+server.ts:1` |

---

## 1. FOUNDATIONAL PRINCIPLES

### 1.1 Minimal Abstraction
- **Direct DB access from server functions**: SvelteKit `+page.server.ts` actions talk directly to Postgres via `pg` — no REST API layer between frontend and database.
  - Examples: `web/src/routes/charts/+page.server.ts:15` (load), `web/src/routes/runs/+page.server.ts:5` (list)
- **No message brokers**: All job queues are plain Postgres tables with fixed-interval polling. No Redis, Kafka, or RabbitMQ.
  - Queues: `build_jobs`, `run_jobs`, `rl_jobs`, `trade_jobs`, `ohlcv_jobs`
- **Simple state machines**: All job statuses follow explicit state flow diagrams (e.g., `pending → building → done / failed`). No hidden state.

### 1.2 Security-First Design
- **Encryption at rest only in Go**: The Go service is the *only* component that holds encryption keys and decrypts broker tokens. Other services never see plaintext tokens.
  - `go/internal/broker/handler.go:27` — AES-256-GCM encryption, `go/internal/broker/token.go:9` — GetToken() decrypts
- **HMAC for WebSocket auth**: WebSocket connections use HMAC-SHA256 signed tokens for authentication.
  - `go/internal/live/handler.go:45` — MakeToken(), `web/src/routes/charts/+page.server.ts:7` — makeWsToken()
- **Correlation IDs for trade deduplication**: Deterministic hashes prevent duplicate trade orders on consecutive ticks.
  - `policy-worker/src/main.rs:477` — record_signal(), truncated to 30 chars

### 1.3 Idempotency & Safety
- **Concurrent-safe job claiming**: Uses `FOR UPDATE SKIP LOCKED` in Postgres for concurrent job claiming.
  - `go/internal/ohlcv/worker.go:262` — claimJob(), 5 worker goroutines with semaphore at :46, staggered 232ms apart at :48
- **Duplicate detection**: Nifty500 sync compares against latest snapshot; trade jobs use correlation IDs.
  - `go/internal/nifty500/sync.go:92` — dedup logic, :110 — archive old data into nifty500_snapshots
- **Retry with limits**: Up to 3 retries with exponential backoff; 400 errors permanently failed.
  - `go/internal/ohlcv/worker.go:411` — retry logic, permanent fail on 400

---

## 2. SERVICE COMMUNICATION PATTERNS

### 2.1 Postgres as Central Bus
- All inter-service communication goes through Postgres tables (job queues) or local HTTP calls.
- Services poll on fixed intervals: **Trade worker** 3s, **Policy worker** 30s, **OHLCV worker** daily, **Run worker** 3s.
  - `builder/src/main.rs:1213` — main loop polls build_jobs, run_jobs, rl_jobs every 3s
  - `go/internal/run/worker.go:23` — Start() polls run_jobs + rl_jobs every 3s
- No event-driven architecture; synchronous polling is the norm.

### 2.2 Go as the Security Boundary
- Go service is the sole intermediary for Dhan API calls (market data, broker tokens).
  - `go/internal/market/handler.go:32` — proxyDhan() injects user's Dhan access token + client ID, rate limited 1 req/s at :27
  - Routes: POST /market/ltp (87), /market/ohlc (88), /market/quote (89) → Dhan /marketfeed/*
- Browser → Go → Dhan for authenticated calls; browser → Postgres for unauthenticated operations.
- Internal HTTP notifications: `POST /internal/broker-token` (broker handler), `POST /internal/ohlcv-trigger` (ohlcv/handler.go:195)

### 2.3 Rust Services as Independent Workers
- **Builder** (`builder/src/main.rs`) — compiles Rust strategy snippets to WASM, runs backtests, trains RL models. MinIO S3 client at :163.
- **Policy Worker** (`policy-worker/src/main.rs`) — loads WASM strategies, processes daily/intraday policies, sends Telegram alerts.
- **Trade Worker** (`trade-worker/src/main.rs`) — polls trade_jobs, executes orders via Dhan API.
- Each polls Postgres, processes jobs, and writes results — no inter-process communication.
- Compiled to WASM (Builder strategies) or native binaries (Worker services).

---

## 3. JOB QUEUE PATTERNS

### 3.1 Status Flow Convention
Every job queue follows an explicit status flow:

| Queue | Status Flow | Key Files |
|-------|-------------|-----------|
| `build_jobs` | `pending → building → done / failed` | `builder/src/main.rs:1228` |
| `run_jobs` | `pending → ready → running → done / failed` | `go/internal/run/worker.go:31` (pollRuns marks ready) |
| `rl_jobs` | `pending → training → done / failed` | `go/internal/run/worker.go:56` (Go marks training), `builder/src/main.rs:1309` (Builder marks done) |
| `trade_jobs` | `pending → checking → placing → polling → done / failed` | `trade-worker/src/main.rs` (full pipeline) |

Terminal trade statuses: TRADED, PART_TRADED, REJECTED, CANCELLED, EXPIRED (`trade-worker/src/main.rs`)

### 3.2 Precondition Checking
- **Candle data readiness**: Before a `run_jobs` can execute, the Go worker ensures all requested candle data exists in Postgres.
  - `go/internal/run/worker.go:110` — fetches candles before marking ready
  - `go/internal/candles/fetch.go:56` — FetchAndStore handles daily + intraday
- **Token refresh triggers**: Policy worker only runs when broker token is refreshed or market hours begin.
- **Build prerequisites**: Builder waits for `build_jobs` to complete before picking up `run_jobs` for that strategy.

### 3.3 Error Handling
- **Permanent failures**: 400 errors from Dhan API are permanently failed (no retry).
  - `go/internal/ohlcv/worker.go:411` — permanent fail on 400
- **Temporary failures**: Network errors trigger up to 3 retries with backoff.
  - `go/internal/ohlcv/worker.go:411-437` — retry logic
- **Error visibility**: Failed jobs store error messages in the database, visible via `/runs/[run_id]` page.
  - `web/src/routes/runs/[run_id]/+page.server.ts:5` — loads error field

---

## 4. STRATEGY & WASM PATTERNS

### 4.1 Strategy Compilation Pipeline
1. User writes a signal function snippet and saves the strategy
2. Builder wraps snippet in a scaffold providing `alloc` and `run` exports
3. Compiles with `cargo build --target wasm32-unknown-unknown`
4. Resulting `.wasm` uploaded to MinIO; strategy record updated with key
  - `builder/src/main.rs:704` — build_parquet() writes to MinIO

### 4.2 WASM Runtime Interface
Every strategy WASM exposes exactly two functions:
- `alloc(len: u32) -> *mut f64` — allocates a price buffer inside WASM memory and returns a pointer
- `run(len: u32) -> *mut u8` — runs the signal logic over the prices and returns a pointer to the signal buffer
- Signals are bytes: 0 = hold, 1 = buy, 2 = sell.
- **Never change these signatures** — all workers (policy, trade, backtest) depend on them.

### 4.3 Indicator System
- **Compiled into every WASM**: All indicators (RSI, SMA, EMA, WMA, MACD, BB, VWAP, ATR, Stochastic, OBV, CCI) are compiled directly into strategy WASM — no dynamic calls between modules.
  - `rust/indicators/src/` — source implementations
  - `rust/indicators-wasm/src/lib.rs:1` — extern "C" exports for browser
  - `web/src/lib/wasm/indicators.ts:1` — TypeScript bindings
- **Rule JSON vs Generated Code**: `strategies.rule_json` stores UI state for visual builder re-hydration; generated Rust code is what actually gets compiled.
  - Editing rule_json does NOT affect compiled WASM; must rebuild strategy.
  - `go/internal/result/handler.go:129` — StrategySource() fetches scaffolded Rust from MinIO for display

### 4.4 RL Training Pipeline
1. **Feature Engineering**: `builder/src/rl/features.rs:63` computes_indicators(), :116 stationary_transform() (EMA distance, RSI centering, BB bandwidth/%B, OBV delta, velocity features), :230 build_state_matrix_with_indices(), :285 normalise_with_stats() (z-score)
2. **Training**: `builder/src/rl/train.rs:14` MLP network (Adam optimizer :99, gradient clipping :463, L1/L2 reg :143). Actor at :166 (discrete: softmax 3 actions, continuous: Gaussian tanh-mean). Critic at :397 (single-output MSE). train_reinforce() at :1206, train_ppo() at :1323 (rayon parallel, GAE, clipped ratio + entropy bonus).
3. **Distillation**: `builder/src/rl/distill.rs:283` net_to_rust() (MLP → Rust arrays), :367 distil() (decision tree), :159 feature_importance() (permutation scoring)
4. **Compilation**: Distilled/generated code scaffolded and compiled to WASM, uploaded to MinIO, same interface as rule-based strategies.

**RL Job Flow**:
1. UI creates strategy + rl_jobs (status: pending) — `web/src/routes/strategies/new/rl/+page.server.ts:39`
2. Go run worker polls rl_jobs, fetches candles, updates status to training — `go/internal/run/worker.go:56`
3. Builder polls rl_jobs, trains, compiles, uploads weights.bin, updates status to done — `builder/src/main.rs:1309`

### 4.5 Indicator Addition Checklist
When adding a new indicator:
1. Implement in `rust/indicators/src/` (e.g., `rsi.rs`)
2. Export via `extern "C"` in `rust/indicators-wasm/src/lib.rs`
3. Add TypeScript bindings in `web/src/lib/wasm/indicators.ts`
4. Add to `IndicatorSpec` enum in `builder/src/rl/features.rs:30`
5. Add stationary transform logic in `stationary_transform()` in `builder/src/rl/features.rs:116`

---

## 5. MARKET DATA PATTERNS

### 5.1 Caching & Reuse
- **Shared candles table**: Historical data stored once, keyed on `(security_id, exchange_segment, interval, timestamp)`. Reused across runs and strategies.
- **Gap filling**: Chart requests fetch from last stored date forward (not just today) to handle multi-day absences. Same refresh-from-last-stored-date logic used by policy worker.
- **90-day chunks**: Dhan API limits to 90 days per request; workers paginate automatically using `FetchChunk` and `FetchAndStore`.
  - `go/internal/candles/fetch.go:147` — single-chunk variant, :56 — full variant with 90-day chunking

### 5.2 Data Freshness
- **Daily OHLCV worker**: Maintains up-to-date daily candle data for all NSE500 stocks.
  - `go/internal/ohlcv/worker.go:31` — Start() creates jobs on boot, :60 next4PMIST() (4 PM IST daily)
  - 5 worker goroutines, staggered 232ms apart to avoid API throttling
- **Instrument master**: Synced from Dhan's scrip master CSV at 9 AM IST daily. UI search queries with prefix-first ordering.
- **Nifty500 sync**: Monthly refresh (1st of month at 9 AM IST) with archival to `nifty500_snapshots`.
  - `go/internal/nifty500/sync.go:171` — RunScheduler(), :13 downloads CSV from archives.nseindia.com

### 5.3 Real-Time Data
- **WebSocket proxy**: Browser → Go → Dhan binary feed at `wss://api-feed.dhan.co`.
  - `go/internal/live/handler.go:24` — connects to Dhan, :170 GET /chart/live upgrades to WebSocket
- **Binary parsing**: 50-byte packets (type 4) parsed for LTP (f32), LTT (u32), Volume (u32) at specific little-endian byte offsets.
  - `go/internal/live/handler.go:86` — parseQuote()
- **Rate limiting**: 1 req/s per user, max 5 concurrent requests on market data API proxies.
  - `go/internal/market/handler.go:27` — rate.Every(1), :28 — max 5 concurrent

### 5.4 Candle Fetching Pattern
- **Daily charts**: Uses `/charts/historical` endpoint via Dhan API, 90-day chunking.
- **Intraday charts**: Uses `/charts/intraday` endpoint, 90-day chunking.
- **Upsert support**: `ON CONFLICT` handling with optional update mode.
  - `go/internal/candles/fetch.go:204` — Upsert with ON CONFLICT

---

## 6. EXECUTION PATTERNS

### 6.1 Policy Execution Lifecycle
1. **Token Refresh Trigger**: Go service notifies policy worker when broker token refreshes
2. **Candle Refresh**: Worker refreshes candles for every instrument in user's active policies (from last stored date to today)
3. **Daily Policies**: Loads close history from Postgres into fresh WASM instance, calls `run()`, fires alert if signal changed
   - `policy-worker/src/main.rs:588` — run_daily_policy()
4. **Intraday Runners**: If during market hours (09:15-15:30 IST), spins up intraday runners with WebSocket connections
   - `policy-worker/src/main.rs:561` — is_market_hours(), :635 — run_intraday_policies()

### 6.2 Intraday Execution Architecture
- One Dhan WebSocket connection per user, subscribed to union of instruments across all active intraday policies.
- Each (policy, instrument) pair holds a persistent wasmtime Store + Instance (`policy-worker/src/main.rs:45` — WasmRunner).
- On every tick: only the last float in WASM memory is updated (live LTP); historical close buffer stays in place.
- `run()` called on every tick; signal transition triggers write to `live_signals` and Telegram message.
- Runners stop when WebSocket closes (market close, token expiry, disconnect).

### 6.3 Trade Execution Pipeline
Full pipeline in `trade-worker/src/main.rs` (polls every 3s):
1. **Value Check**: `:248` — quantity x price vs max_trade_value on policy_instruments. Exceeds = reject + Telegram alert
2. **Margin Check**: `:94` — calls Dhan /margincalculator. Insufficient balance = reject + alert
3. **Fund Limit Check**: `:54` — calls Dhan /fundlimit
4. **Position Check**: Same direction position exists = skip. Opposite direction = close first
5. **Order Placement**: Calls Dhan /orders with correlationId (truncated to 30 chars). Product type: INTRADAY for intraday, CNC for daily
6. **Status Polling**: Up to 20 polls (3s apart) until terminal status: TRADED, PART_TRADED, REJECTED, CANCELLED, EXPIRED
7. **Position Recording**: On fill = write/update trade_positions. On failure = mark failed + Telegram alert

### 6.4 Alert Delivery
- Writes row to `live_signals` (policy, instrument, signal, price, timestamp)
- Calls Telegram Bot API with user's telegram_chat_id
- Policy worker talks to Telegram directly (Go service not involved in signal path)
- Deterministic correlation_id (hash of policy, instrument, signal direction, price) prevents duplicates

### 6.5 EOD Force-Close
- **RL Training**: `builder/src/rl/train.rs:831` — day_boundaries check, force-close at end of trading day for intraday intervals
- **Backtest**: `builder/src/main.rs:319` — detects day change in timestamps, force-closes position
- **Policy Worker**: `policy-worker/src/main.rs:761` — queries trade_positions for open intraday positions, queues MARKET close orders with `-eod-close` suffix

### 6.6 Position Sizing & Deadband
- **Policy Instruments**: `policy-worker/src/main.rs:28` — Instrument struct with quantity field
- **Position Deadband**: `builder/src/main.rs:107` — const POSITION_DEADBAND: f64 = 0.05, `builder/src/rl/train.rs:571` — TrainConfig.position_deadband default 0.05
  - Used in step_reward() at :682, evaluate() at :1544

---

## 7. DATA PERSISTENCE PATTERNS

### 7.1 MinIO Artifacts
| Artifact | Location | Producer | Consumer |
|----------|----------|----------|----------|
| .wasm | strategies/{strategy_id}.wasm | Builder | Run worker, Policy worker |
| signals.json | runs/{run_id}/signals.json | Builder (`builder/src/main.rs:704`) | Go service API proxy (`go/internal/result/handler.go:41`) |
| result.parquet | runs/{run_id}/result.parquet | Builder | Go service (delete endpoint) |
| weights | rl/{run_id}/weights.bin | Builder | Distillation, codegen |

### 7.2 Postgres Tables by Concern
| Concern | Tables | Key Files |
|---------|--------|-----------|
| Auth | users, broker_connections, telegram_link_tokens | `web/src/routes/auth/google/`, `go/internal/broker/` |
| Strategy | strategies, build_jobs | `builder/src/main.rs:1228` |
| Backtest | backtest_runs, backtest_run_instruments, run_jobs | `web/src/routes/runs/`, `go/internal/result/handler.go` |
| RL | rl_jobs, rl_training_metrics | `go/internal/run/worker.go:56`, `builder/src/main.rs:1309` |
| Execution | policies, policy_instruments, trade_jobs, trade_positions | `policy-worker/src/main.rs`, `trade-worker/src/main.rs` |
| Market Data | candles, instruments, nifty500_constituents, ohlcv_jobs | `go/internal/ohlcv/worker.go`, `go/internal/nifty500/sync.go` |
| UI | charts | `web/src/routes/charts/+page.server.ts` |

### 7.3 Delete Cascades
- Deleting a run removes both Postgres row and MinIO artifacts (parquet + signals.json).
  - `web/src/routes/runs/+page.server.ts:40` — frontend delete action, calls Go API
  - `go/internal/result/handler.go:94` — DeleteRun() removes both parquet and signals.json
- Chart deletion is user-scoped to the charts table.
  - `web/src/routes/charts/+page.server.ts:65` — delete scoped to user
- Strategy deletion cascades to build jobs and MinIO WASM files.

### 7.4 JWT Session Pattern
- HS256 JWT with 30-day expiry using jose library.
  - `web/src/lib/server/session.ts:1`
- Session middleware in `web/src/hooks.server.ts:5` verifies JWT on every request, loads user into `event.locals.user`.
- Google OAuth: `web/src/routes/auth/google/callback/+server.ts:8` — validates ID token claims, upserts into users table.
  - Scopes: openid, email, profile (`web/src/lib/server/oauth.ts:9`)
  - PKCE flow with generateState() and generateCodeVerifier(), state/verifier in httpOnly cookies

---

## 8. UI/UX PATTERNS

### 8.1 Server-Side State
- **JWT sessions**: 30-day HS256 JWT issued on Google OAuth callback.
- **Svelte stores**: `web/src/lib/stores/auth.ts:1` — user and brokerConnected writable stores.
- **No client-side DB**: All data loading happens in `+page.server.ts` functions.
- **Forms submit directly to server actions**: No REST API between frontend and database.
  - `web/src/routes/charts/+page.server.ts:28` — save action
  - `web/src/routes/runs/+page.server.ts:5` — list action

### 8.2 Indicator Persistence
- Saved charts store indicators as JSON array in charts table.
  - `web/src/routes/charts/+page.server.ts:15` — load, :42 validates JSON, :28 — insert/update
- On load, indicator configs are re-hydrated for exact visual reproduction.
- Chart page renders with exact same indicators and settings as saved.

### 8.3 WebSocket Token Generation
- HMAC-SHA256 tokens generated server-side for WebSocket authentication.
  - `go/internal/live/handler.go:45` — MakeToken()
  - `web/src/routes/charts/+page.server.ts:7` — makeWsToken()
- Tokens scoped to user and timestamp, preventing reuse.
- Admin WebSocket status endpoint uses HMAC-signed token validation.

### 8.4 Strategy Creation Flow
- **Rule builder**: Visual interface creates buy/sell conditions using indicators.
- **RL training**: `web/src/routes/strategies/new/rl/+page.server.ts:1` — UI configures training params, creates strategy + rl_jobs entry.
- Both flows result in WASM compilation and same execution interface (alloc/run).

---

## 9. OPERATIONAL PATTERNS

### 9.1 Scheduling
- **Boot-time syncs**: Nifty500 initial sync (`go/internal/nifty500/sync.go:171`), OHLCV job creation (`go/internal/ohlcv/worker.go:31`).
- **Daily schedules**: Instrument master (9 AM IST), OHLCV worker (4 PM IST via `:60 next4PMIST()`), Nifty500 (1st of month).
- **Market-hour awareness**: `policy-worker/src/main.rs:561` — is_market_hours() checks 9:15-15:30 IST.
- **Fixed interval polling**: Trade worker (3s), Policy worker (30s), OHLCV worker (scheduled), Builder/Run worker (3s).

### 9.2 Resource Management
- **Candle reuse**: Shared candles table prevents duplicate fetches across runs.
- **WASM instance pooling**: One wasmtime::Store + Instance per (policy, instrument) pair for intraday (`policy-worker/src/main.rs:45`).
- **Concurrency limits**: 5 goroutines for OHLCV worker (`go/internal/ohlcv/worker.go:46`), 5 concurrent Dhan API requests per user (`go/internal/market/handler.go:28`).
- **Staggered processing**: OHLCV workers staggered 232ms apart to avoid API throttling (`go/internal/ohlcv/worker.go:48`).

### 9.3 Monitoring & Debugging
- **Admin endpoints** (`go/internal/ohlcv/handler.go:189`):
  - GET /admin/ohlcv — stats
  - GET /admin/ohlcv/ws — WebSocket live progress (HMAC-secured)
  - POST /admin/ohlcv/trigger — admin trigger
  - GET /ohlcv/stocks — stock list with pagination
  - POST /internal/ohlcv-trigger — internal trigger (from broker handler notification)
- **Admin page**: `web/src/routes/admin/+page.server.ts:14` — checks locals.user.id === OHLCV_USER_ID
- **Run details**: Error messages visible on `/runs/[run_id]` page.
- **Job status polling**: All queues visible via status fields in respective tables.
- **Strategy source retrieval**: Go service fetches scaffolded Rust source from MinIO for display (`go/internal/result/handler.go:129`).

---

## 10. DECISION-MAKING GUIDELINES

### 10.1 When Adding a New Service
1. **Postgres first**: Can you use a job queue instead of a new service?
2. **Go boundary**: Does it need Dhan API access? If yes, add to Go service.
3. **Rust worker**: Is it a fire-and-forget background task? Consider a new Rust worker (Builder, Policy Worker, or Trade Worker).
4. **SvelteKit**: Is it UI-driven? Add to `+page.server.ts` actions (e.g., `web/src/routes/charts/+page.server.ts`, `web/src/routes/runs/+page.server.ts`).

### 10.2 When Modifying Existing Flows
1. **Maintain status flows**: Do not add new statuses without updating the state diagram (Section 3.1).
2. **Preserve WASM interface**: `alloc()` and `run()` signatures must not change — all workers depend on them.
3. **Keep encryption in Go**: Never expose broker tokens to other services (`go/internal/broker/handler.go:27`).
4. **Idempotency first**: Ensure new operations can be safely retried (`FOR UPDATE SKIP LOCKED`).

### 10.3 When Adding Indicators
1. Implement in `rust/indicators/src/` (e.g., `rsi.rs`)
2. Export via `extern "C"` in `rust/indicators-wasm/src/lib.rs`
3. Add TypeScript bindings in `web/src/lib/wasm/indicators.ts`
4. Add to `IndicatorSpec` enum in `builder/src/rl/features.rs:30`
5. Add stationary transform logic in `stationary_transform()` in `builder/src/rl/features.rs:116`

### 10.4 When Adding Trade Modes
1. Update trade worker with new status transitions to trade_jobs flow
2. Add pre-checks in order: value check, margin check, position check
3. Add Telegram alerts for rejected jobs
4. Ensure deterministic correlation_id hashing for deduplication

---

## 11. COMMON PITFALLS

- **Forgetting candle readiness**: Run jobs will fail if candle data isn't fetched first by the Go worker (`go/internal/run/worker.go:110`).
- **WASM memory leaks**: Always match `alloc()` with proper `run()` calls; WASM memory is isolated per instance.
- **Token expiry**: Broker tokens expire daily; policies stop working until user reconnects.
- **Market hours assumptions**: Do not assume intraday runners are always running; they stop at market close (`policy-worker/src/main.rs:761`).
- **MinIO deletion**: Always delete both parquet and signals.json when removing a run via Go API (`go/internal/result/handler.go:94`).
- **Indicator normalization**: RL training requires z-score normalization via `normalise_with_stats()` (`builder/src/rl/features.rs:285`); do not skip.
- **Binary WebSocket parsing**: Dhan packets are little-endian; wrong offsets break LTP extraction (`go/internal/live/handler.go:86`).
- **Rate limiting**: Dhan API enforces 1 req/s per user; exceed and you will get throttled (`go/internal/market/handler.go:27`).
- **Correlation ID collisions**: Ensure deterministic hashing of (policy, instrument, direction, price) for trade deduplication.
- **Rule JSON vs Generated Code**: Editing `rule_json` does not affect compiled WASM; must rebuild strategy.
- **WASM compilation target**: Strategies must compile to `wasm32-unknown-unknown`; adding std feature will break.
- **Indicators baked into WASM**: Cannot add indicators at runtime; must update indicators crate and recompile.
