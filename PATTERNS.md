This document describes the patterns any developer or agent should keep in mind while updating this repository. It is organized as a tree of increasingly bigger concepts — from foundational principles to high-level system patterns.

---

## 1. FOUNDATIONAL PRINCIPLES

### 1.1 Minimal Abstraction
- **Direct DB access from server functions**: SvelteKit `+page.server.ts` actions talk directly to Postgres via `pg` — no REST API layer between frontend and database.
- **No message brokers**: All job queues are plain Postgres tables with fixed-interval polling. No Redis, Kafka, or RabbitMQ.
- **Simple state machines**: All job statuses follow explicit state flow diagrams (e.g., `pending → building → done / failed`). No hidden state.

### 1.2 Security-First Design
- **Encryption at rest only in Go**: The Go service is the *only* component that holds encryption keys and decrypts broker tokens. Other services never see plaintext tokens.
- **HMAC for WebSocket auth**: WebSocket connections use HMAC-SHA256 signed tokens for authentication.
- **Correlation IDs for trade deduplication**: Deterministic hashes prevent duplicate trade orders on consecutive ticks.

### 1.3 Idempotency & Safety
- **Concurrent-safe job claiming**: Uses `FOR UPDATE SKIP LOCKED` in Postgres for concurrent job claiming.
- **Duplicate detection**: Nifty500 sync compares against latest snapshot; trade jobs use correlation IDs.
- **Retry with limits**: Up to 3 retries with exponential backoff; 400 errors permanently failed.

---

## 2. SERVICE COMMUNICATION PATTERNS

### 2.1 Postgres as Central Bus
- All inter-service communication goes through Postgres tables (job queues) or local HTTP calls.
- Services poll on fixed intervals (3s for trade worker, 30s for policy worker, daily for OHLCV worker).- No event-driven architecture; synchronous polling is the norm.

### 2.2 Go as the Security Boundary
- Go service is the sole intermediary for Dhan API calls (market data, broker tokens).
- Browser → Go → Dhan for authenticated calls; browser → Postgres for unauthenticated operations.
- Local HTTP calls only for internal notifications (e.g., `POST /internal/user-connected` to policy worker).

### 2.3 Rust Services as Independent Workers
- Builder, Policy Worker, and Trade Worker are independent Rust binaries.
- Each polls Postgres, processes jobs, and writes results — no inter-process communication.
- Compiled to WASM (Builder) or native binaries (Worker services).

---

## 3. JOB QUEUE PATTERNS

### 3.1 Status Flow Convention
Every job queue follows an explicit status flow:

| Queue | Status Flow |
|-------|-------------|
| `build_jobs` | `pending → building → done / failed` |
| `run_jobs` | `pending → ready → running → done / failed` |
| `rl_jobs` | `pending → training → running → done / failed` |
| `trade_jobs` | `pending → checking → placing → polling → done / failed` |

### 3.2 Precondition Checking
- **Candle data readiness**: Before a `run_jobs` can execute, the Go worker ensures all requested candle data exists in Postgres.
- **Token refresh triggers**: Policy worker only runs when broker token is refreshed or market hours begin.
- **Build prerequisites**: Builder waits for `build_jobs` to complete before picking up `run_jobs` for that strategy.

### 3.3 Error Handling
- **Permanent failures**: 400 errors from Dhan API are permanently failed (no retry).
- **Temporary failures**: Network errors trigger up to 3 retries with backoff.
- **Error visibility**: Failed jobs store error messages in the database, visible via `/runs/[run_id]` page.

---

## 4. STRATEGY & WASM PATTERNS

### 4.1 Strategy Compilation Pipeline
1. User writes a signal function snippet and saves the strategy
2. Builder wraps snippet in a scaffold providing `alloc` and `run` exports
3. Compiles with `cargo build --target wasm32-unknown-unknown`
4. Resulting `.wasm` uploaded to MinIO; strategy record updated with key

### 4.2 WASM Runtime Interface
Every strategy WASM exposes exactly two functions:
- `alloc(len: u32) -> *mut f64` — allocates a price buffer inside WASM memory and returns a pointer
- `run(len: u32) -> *mut u8` — runs the signal logic over the prices and returns a pointer to the signal buffer

Signals are bytes: 0 = hold, 1 = buy, 2 = sell.

### 4.3 Indicator System
- **Compiled into every WASM**: All indicators (RSI, SMA, EMA, WMA, MACD, BB, VWAP, ATR, Stochastic, OBV, CCI) are compiled directly into strategy WASM — no dynamic calls between modules.
- **Browser rendering**: Separate `indicators-wasm` crate exports same functions as `extern "C"` for TypeScript bindings in `web/src/lib/wasm/indicators.ts`.
- **Rule JSON vs Generated Code**: `strategies.rule_json` stores UI state for visual builder re-hydration; generated Rust code is what actually gets compiled.

### 4.4 RL Training Pipeline
1. **Feature Engineering**: Computes indicators over historical OHLCV data, applies stationary transforms (EMA distance, RSI centering, BB bandwidth/%B, OBV delta normalization, velocity features), combines with OHLCV log returns, and z-score normalizes via `normalise_with_stats()`.
2. **Training**: Trains Actor-Critic MLP using PPO or REINFORCE. Actor outputs discrete actions (hold/buy/sell via softmax) or continuous actions (Gaussian with tanh-mean). Critic is single-output MLP with MSE loss. Uses Adam optimizer, gradient clipping, L1/L2 regularization. PPO uses clipped ratio with entropy bonus and GAE.
3. **Distillation**: Decision tree distillation from trained actor via `distil()`. Feature importance via permutation-based scoring. MLP weights converted to Rust arrays via `net_to_rust()`.
4. **Compilation**: Distilled/generated Rust code is scaffolded and compiled to WASM, uploaded to MinIO, same interface as rule-based strategies.

### 4.5 Indicator Addition Checklist
When adding a new indicator:
1. Implement in `rust/indicators/src/` (e.g., `rsi.rs`)
2. Export via `extern "C"` in `rust/indicators-wasm/src/lib.rs`
3. Add TypeScript bindings in `web/src/lib/wasm/indicators.ts`
4. Add to `IndicatorSpec` enum in `builder/src/rl/features.rs`
5. Add stationary transform logic in `stationary_transform()` in `builder/src/rl/features.rs`

---

## 5. MARKET DATA PATTERNS

### 5.1 Caching & Reuse
- **Shared candles table**: Historical data stored once, keyed on `(security_id, exchange_segment, interval, timestamp)`. Reused across multiple runs and strategies.
- **Gap filling**: Chart requests fetch from last stored date forward (not just today) to handle multi-day absences. Same refresh-from-last-stored-date logic used by policy worker.
- **90-day chunks**: Dhan API limits to 90 days per request; workers paginate automatically using `FetchChunk` and `FetchAndStore`.

### 5.2 Data Freshness
- **Daily OHLCV worker**: Maintains up-to-date daily candle data for all NSE500 stocks. Runs on boot, then daily at 4:00 PM IST.
- **Instrument master**: Synced from Dhan's scrip master CSV at 9 AM IST daily. UI search queries this with prefix-first ordering.
- **Nifty500 sync**: Monthly refresh (1st of month at 9 AM IST) with archival to `nifty500_snapshots`. Duplicate detection compares against latest snapshot.

### 5.3 Real-Time Data
- **WebSocket proxy**: Browser → Go → Dhan binary feed at `wss://api-feed.dhan.co`.
- **Binary parsing**: 50-byte packets (type 4) parsed for LTP (f32), LTT (u32), Volume (u32) at specific little-endian byte offsets.
- **Intraday runners**: One WebSocket per user, subscribed to union of policy instruments (up to 5000 per connection; Dhan allows 5 concurrent connections per user).
- **Rate limiting**: 1 req/s per user, max 5 concurrent requests on market data API proxies.

### 5.4 Candle Fetching Pattern
- **Daily charts**: Uses `/charts/historical` endpoint via Dhan API, 90-day chunking.
- **Intraday charts**: Uses `/charts/intraday` endpoint, 90-day chunking.
- **Upsert support**: `ON CONFLICT` handling with optional update mode to prevent duplicate inserts.

---

## 6. EXECUTION PATTERNS

### 6.1 Policy Execution Lifecycle
1. **Token Refresh Trigger**: Go service calls `POST /internal/user-connected` when broker token refreshes
2. **Candle Refresh**: Worker refreshes candles for every instrument in user's active policies (from last stored date to today)
3. **Daily Policies**: Loads close history from Postgres into fresh WASM instance, calls `run()`, fires alert if signal changed
4. **Intraday Runners**: If during market hours (09:15-15:30 IST), spins up intraday runners with WebSocket connections

### 6.2 Intraday Execution Architecture
- One Dhan WebSocket connection per user, subscribed to union of instruments across all active intraday policies
- Each (policy, instrument) pair holds a persistent wasmtime Store + Instance
- On every tick: only the last float in WASM memory is updated (live LTP); historical close buffer stays in place
- `run()` called on every tick; signal transition triggers write to `live_signals` and Telegram message
- Runners stop when WebSocket closes (market close, token expiry, disconnect)

### 6.3 Trade Execution Pipeline
1. **Value Check**: quantity x price vs max_trade_value on policy_instruments. Exceeds = reject + Telegram alert
2. **Margin Check**: Calls Dhan /margincalculator with exact order params. Insufficient balance = reject + alert
3. **Position Check**: Same direction position exists = skip. Opposite direction = close first
4. **Order Placement**: Calls Dhan /orders with correlationId (truncated to 30 chars). Product type: INTRADAY for intraday, CNC for daily
5. **Status Polling**: Up to 20 polls (3s apart) until terminal status: TRADED, PART_TRADED, REJECTED, CANCELLED, EXPIRED
6. **Position Recording**: On fill = write/update trade_positions. On failure = mark failed + Telegram alert

### 6.4 Alert Delivery
- Writes row to `live_signals` (policy, instrument, signal, price, timestamp)
- Calls Telegram Bot API with user's telegram_chat_id
- Policy worker talks to Telegram directly (Go service not involved in signal path)
- Deterministic correlation_id (hash of policy, instrument, signal direction, price) prevents duplicates on consecutive ticks

---

## 7. DATA PERSISTENCE PATTERNS

### 7.1 MinIO Artifacts
| Artifact | Location | Producer | Consumer |
|----------|----------|----------|----------|
| .wasm | strategies/{strategy_id}.wasm | Builder | Run worker, Policy worker |
| signals.json | runs/{run_id}/signals.json | Builder | Go service (API proxy) |
| result.parquet | runs/{run_id}/result.parquet | Builder | Go service (delete endpoint) |
| weights | rl/{run_id}/weights.bin | Builder | Distillation, codegen |

### 7.2 Postgres Tables by Concern
| Concern | Tables |
|---------|--------|
| Auth | users, broker_connections, telegram_link_tokens |
| Strategy | strategies, build_jobs |
| Backtest | backtest_runs, backtest_run_instruments, run_jobs |
| RL | rl_jobs, rl_training_metrics |
| Execution | policies, policy_instruments, trade_jobs, trade_positions |
| Market Data | candles, instruments, nifty500_constituents, ohlcv_jobs |
| UI | charts |

### 7.3 Delete Cascades
- Deleting a run removes both Postgres row and MinIO artifacts (parquet + signals.json)
- Chart deletion is user-scoped to the charts table
- Strategy deletion cascades to build jobs and MinIO WASM files
- Delete action on runs calls Go API `DELETE /result/run/{run_id}` to remove MinIO objects first

### 7.4 JWT Session Pattern
- HS256 JWT with 30-day expiry using jose library
- Session middleware in hooks.server.ts verifies JWT on every request
- Populates event.locals.user for downstream use in SvelteKit actions
- Google OAuth issues JWT after validating ID token claims (openid, email, profile scopes)

---

## 8. UI/UX PATTERNS

### 8.1 Server-Side State
- **JWT sessions**: 30-day HS256 JWT issued on Google OAuth callback
- **Svelte stores**: auth.ts for user and brokerConnected state
- **No client-side DB**: All data loading happens in +page.server.ts functions
- **Forms submit directly to server actions**: No REST API between frontend and database

### 8.2 Indicator Persistence
- Saved charts store indicators as JSON array in charts table
- On load, indicator configs are re-hydrated for exact visual reproduction
- Validation ensures only valid JSON indicator configs are saved
- Chart page renders with exact same indicators and settings as saved

### 8.3 WebSocket Token Generation
- HMAC-SHA256 tokens generated server-side for WebSocket authentication
- Tokens scoped to user and timestamp, preventing reuse
- Admin WebSocket status endpoint uses HMAC-signed token validation

### 8.4 Strategy Creation Flow
- Rule builder: Visual interface creates buy/sell conditions using indicators
- RL training: UI configures training parameters, creates strategy + rl_jobs entry
- Both flows result in WASM compilation and same execution interface

---

## 9. OPERATIONAL PATTERNS

### 9.1 Scheduling
- **Boot-time syncs**: Nifty500 initial sync, OHLCV job creation
- **Daily schedules**: Instrument master (9 AM IST), OHLCV worker (4 PM IST), Nifty500 (1st of month)
- **Market-hour awareness**: Policies check IST time to determine intraday runner behavior
- **Fixed interval polling**: Trade worker (3s), Policy worker (30s), OHLCV worker (scheduled)

### 9.2 Resource Management
- **Candle reuse**: Shared candles table prevents duplicate fetches across runs
- **WASM instance pooling**: One wasmtime::Store + Instance per (policy, instrument) pair for intraday
- **Concurrency limits**: 5 goroutines for OHLCV worker, 5 concurrent Dhan API requests per user
- **Staggered processing**: OHLCV workers staggered 232ms apart to avoid API throttling

### 9.3 Monitoring & Debugging
- **Admin endpoints**: /admin/ohlcv for stats, /admin/ohlcv/ws for WebSocket status (HMAC-secured)
- **Run details**: Error messages visible on /runs/[run_id] page
- **Job status polling**: All queues visible via status fields in respective tables
- **Strategy source retrieval**: Go service fetches scaffolded Rust source from MinIO for display

---

## 10. DECISION-MAKING GUIDELINES

### 10.1 When Adding a New Service
1. **Postgres first**: Can you use a job queue instead of a new service?
2. **Go boundary**: Does it need Dhan API access? If yes, add to Go service.
3. **Rust worker**: Is it a fire-and-forget background task? Consider a new Rust worker.
4. **SvelteKit**: Is it UI-driven? Add to +page.server.ts actions.

### 10.2 When Modifying Existing Flows
1. **Maintain status flows**: Do not add new statuses without updating the state diagram.
2. **Preserve WASM interface**: alloc() and run() signatures must not change.
3. **Keep encryption in Go**: Never expose broker tokens to other services.
4. **Idempotency first**: Ensure new operations can be safely retried.

### 10.3 When Adding Indicators
1. Implement in rust/indicators/src/ (e.g., rsi.rs)
2. Export via extern "C" in rust/indicators-wasm/src/lib.rs
3. Add TypeScript bindings in web/src/lib/wasm/indicators.ts
4. Add to IndicatorSpec enum in builder/src/rl/features.rs
5. Add stationary transform logic in stationary_transform() in builder/src/rl/features.rs

### 10.4 When Adding Trade Modes
1. Update trade worker with new status transitions to trade_jobs flow
2. Add pre-checks in order: value check, margin check, position check
3. Add Telegram alerts for rejected jobs
4. Ensure deterministic correlation_id hashing for deduplication

---

## 11. COMMON PITFALLS

- **Forgetting candle readiness**: Run jobs will fail if candle data isn't fetched first by the Go worker.
- **WASM memory leaks**: Always match alloc() with proper run() calls; WASM memory is isolated per instance.
- **Token expiry**: Broker tokens expire daily; policies stop working until user reconnects.
- **Market hours assumptions**: Do not assume intraday runners are always running; they stop at market close.
- **MinIO deletion**: Always delete both parquet and signals.json when removing a run via Go API.
- **Indicator normalization**: RL training requires z-score normalization via normalise_with_stats(); do not skip.
- **Binary WebSocket parsing**: Dhan packets are little-endian; wrong offsets break LTP extraction.
- **Rate limiting**: Dhan API enforces 1 req/s per user; exceed and you will get throttled.
- **Correlation ID collisions**: Ensure deterministic hashing of (policy, instrument, direction, price) for trade deduplication.
- **Rule JSON vs Generated Code**: Editing rule_json does not affect compiled WASM; must rebuild strategy.
- **WASM compilation target**: Strategies must compile to wasm32-unknown-unknown; adding std feature will break.
- **Indicators baked into WASM**: Cannot add indicators at runtime; must update indicators crate and recompile.
