1. AUTHENTICATION
Google OAuth
- Route: GET /auth/google -- /Users/madhavkandukuri/GitHub/madhav165/dhan-test/web/src/routes/auth/google/+server.ts (lines 6-15)
- Callback: GET /auth/google/callback -- web/src/routes/auth/google/callback/+server.ts (lines 8-36)
- OAuth Client: web/src/lib/server/oauth.ts (line 4) -- uses arctic library with Google provider
- Scopes: openid, email, profile (line 9)
- Flow: PKCE with generateState() and generateCodeVerifier(), stores state/verifier in httpOnly cookies (lines 11-12), validates ID token claims and upserts into users table (lines 21-25)
Dhan OAuth (Broker Connection)
- Init Route: GET /auth/dhan -- web/src/routes/auth/dhan/+server.ts (lines 6-33)
- Callback Route: GET /auth/dhan/callback -- web/src/routes/auth/dhan/callback/+server.ts (lines 5-39)
- Flow: 
  - Reads client_id from query param or existing broker_connections table (lines 10-17)
  - Calls Dhan consent API: POST /app/generate-consent to get consentAppId (line 22)
  - Redirects to Dhan consent login (line 32)
  - Callback: exchanges tokenId for dhanClientId + accessToken via POST /app/consumeApp-consent (line 13)
  - Stores encrypted token to Go backend via POST /internal/broker-token (lines 22-34)
- Token Storage: go/internal/broker/handler.go (lines 27-61) -- AES-256-GCM encrypted in broker_connections table
- Token Retrieval: go/internal/broker/token.go (lines 9-28) -- GetToken() decrypts and returns client_id + access_token
Session Management
- JWT Session: web/src/lib/server/session.ts (lines 1-20) -- HS256 JWT with 30d expiry using jose
- Hooks: web/src/hooks.server.ts (lines 5-19) -- verifies session on every request, loads user into event.locals.user
- Auth Store: web/src/lib/stores/auth.ts (lines 1-9) -- Svelte writable stores for user and brokerConnected
2. MARKET DATA
Nifty500 Constituents
- Sync: go/internal/nifty500/sync.go (lines 1-195)
  - Downloads CSV from https://archives.nseindia.com/content/indices/ind_nifty500list.csv (line 13)
  - RunScheduler() (line 171): initial sync at boot, then schedules for 1st of every month at 9:00 AM IST
  - Archives old data into nifty500_snapshots before wiping nifty500_constituents (lines 110-124)
  - Deduplicates by comparing against latest snapshot (lines 92-101)
- API Endpoint: GET /nifty500 -- go/internal/nifty500/handler.go (lines 21-48)
OHLCV Background Worker
- Worker: go/internal/ohlcv/worker.go (lines 1-437)
  - Start() (line 31): creates jobs on boot, then schedules daily at 4:00 PM IST (next4PMIST(), line 60)
  - 5 worker goroutines with semaphore (line 46), staggered 232ms apart (line 48)
  - claimJob() (line 262): uses FOR UPDATE SKIP LOCKED for concurrent-safe job claiming
  - createJobs() (line 118): fetches NSE_E stocks from nse500_extended joined with instruments, splits into 90-day chunks (line 191)
  - Rate-limited via per-user token bucket (line 231-237)
  - Retry logic: up to 3 retries with backoff, permanent fail on 400 errors (lines 411-437)
- WebSocket Status: GET /admin/ohlcv/ws -- HandleStatusWS() (lines 348-408), HMAC-signed token validation
- Admin Stats: GET /admin/ohlcv -- HandleStats() (lines 15-73)
Candle Fetching
- FetchAndStore: go/internal/candles/fetch.go (line 56) -- handles both daily (/charts/historical) and intraday (/charts/intraday) via Dhan API, 90-day chunking
- FetchChunk: go/internal/candles/fetch.go (line 147) -- single-chunk variant for the worker
- Upsert: go/internal/candles/fetch.go (lines 204-239) -- ON CONFLICT support with optional update mode
API Proxies (LTP/OHLC/Quotes)
- Handler: go/internal/market/handler.go (lines 1-90)
  - proxyDhan() (line 32): generic reverse-proxy that injects user's Dhan access token and client ID
  - Rate limited: 1 req/s per user (rate.Every(1), line 27), max 5 concurrent (line 28)
- Routes:
  - POST /market/ltp -> Dhan /marketfeed/ltp (line 87)
  - POST /market/ohlc -> Dhan /marketfeed/ohlc (line 88)
  - POST /market/quote -> Dhan /marketfeed/quote (line 89)
WebSocket Live Chart Feed
- Handler: go/internal/live/handler.go (lines 1-171)
  - GET /chart/live (line 170) -- upgrades to WebSocket
  - Proxies Dhan binary feed: connects to wss://api-feed.dhan.co (line 24), subscribes to instrument (RequestCode 17, line 137)
  - parseQuote() (line 86): parses 50-byte binary packet (type 4) extracting LTP (f32), LTT (u32), Volume (u32) from little-endian bytes at specific offsets
  - MakeToken() (line 45): HMAC-SHA256 signed token for WebSocket auth
3. STRATEGY / INDICATORS
Rust Indicators (all in rust/indicators/src/)
Indicator	File	Function
WMA	wma.rs:1	wma(prices, period) -- weighted moving average
ATR	atr.rs:4	atr(highs, lows, closes, period) -- uses ta::AverageTrueRange
Stochastic	stoch.rs:4	stoch(highs, lows, closes, period) -- uses ta::FastStochastic
OBV	obv.rs:4	obv(closes, volumes) -- uses ta::OnBalanceVolume
CCI	cci.rs:4	cci(highs, lows, closes, period) -- uses ta::CommodityChannelIndex
RSI	rsi.rs	(also available)
SMA	sma.rs	(also available)
EMA	ema.rs	(also available)
MACD	macd.rs	(also available)
Bollinger Bands	bb.rs	(also available)
VWAP	vwap.rs	(also available)
WASM Exports for Browser
- File: rust/indicators-wasm/src/lib.rs (lines 1-127) -- extern "C" functions: sma_run, ema_run, wma_run, rsi_run, macd_run, bb_run, vwap_run, atr_run, stoch_run, obv_run, cci_run
- TypeScript Bindings: web/src/lib/wasm/indicators.ts (lines 1-140) -- loadIndicators(), runIndicator(), runMacd(), runBB(), runVwap(), runAtr(), runStoch(), runObv(), runCci()
RL Training System
- MLP Network: builder/src/rl/train.rs (lines 14-163)
  - Configurable: input_size, hidden_size, num_layers, activation (Tanh/ReLU)
  - He initialization based on activation type (lines 32-35)
  - Adam optimizer (line 99), gradient clipping (line 463), L1/L2 regularization (line 143)
- Actor (Policy): builder/src/rl/train.rs (lines 166-395)
  - Discrete: 3 actions (hold/buy/sell) with softmax output (line 188-192)
  - Continuous: Gaussian with tanh-mean, configurable action_std (lines 185-187, 207-215)
  - REINFORCE gradient: accumulate_grad() (line 234)
  - PPO gradient: accumulate_grad_ppo() (line 298) with clipped ratio and entropy bonus
- Critic (Value Network): builder/src/rl/train.rs (lines 397-444) -- single-output MLP, MSE loss
- Training Functions:
  - train_reinforce(): builder/src/rl/train.rs line 1206 -- full REINFORCE with baseline returns
  - train_ppo(): builder/src/rl/train.rs line 1323 -- PPO with GAE, parallel rollouts (rayon into_par_iter), minibatch training
- Distillation: builder/src/rl/distill.rs (lines 1-376)
  - net_to_rust() (line 283): generates Rust code from trained MLP weights (baked-in arrays)
  - distil() (line 367): decision tree distillation from trained actor
  - feature_importance() (line 159): permutation-based importance scoring
  - codegen_transforms() (line 37): stationary transforms + velocity features
RL Feature Engineering
- File: builder/src/rl/features.rs (lines 1-320)
  - IndicatorSpec enum (line 30): all indicator types as deserializable config
  - compute_indicators() (line 63): dispatches to all Rust indicator functions
  - stationary_transform() (line 116): EMA distance, RSI centering, BB bandwidth/%B, OBV delta normalization, velocity features
  - build_state_matrix_with_indices() (line 230): indicator values + OHLCV log returns (5 features x lookback)
  - normalise_with_stats() (line 285): z-score normalization
RL Config UI Types
- File: web/src/lib/types/rl.ts (lines 1-76) -- RLConfig, RLSummary, RLReward, RLConstraint types
- RL Strategy Creation: web/src/routes/strategies/new/rl/+page.server.ts (lines 1-46) -- creates strategy + rl_jobs entry
4. UI/UX: SAVED CHARTS WITH INDICATOR PERSISTENCE
- Page: web/src/routes/charts/+page.server.ts (lines 1-71)
- Load: Queries charts table for user's saved charts with indicators JSON column (lines 15-25)
- Save Action (lines 28-63): 
  - Inserts or updates charts table with name, security_id, exchange_segment, interval, and indicators (JSON array of indicator configs)
  - Validates indicators as valid JSON (lines 42-46)
- Delete Action (lines 65-70): removes chart by id, scoped to user
- WebSocket Token: makeWsToken() (lines 7-12) generates HMAC token for live feed
5. RUN MANAGEMENT
CRUD Operations
- List Runs: web/src/routes/runs/+page.server.ts (lines 5-37) -- lists backtest_runs with strategy name, job status, symbols, PnL metrics
- Create Run: web/src/routes/runs/new/+page.server.ts (lines 21-60) -- inserts into backtest_runs, backtest_run_instruments, and run_jobs
- View Run: web/src/routes/runs/[run_id]/+page.server.ts (lines 5-33) -- run details with instruments, job status/error
- Delete Run: web/src/routes/runs/+page.server.ts action (lines 40-52) -- deletes MinIO objects via Go API then DB row
- Strategy Source: go/internal/result/handler.go StrategySource() (lines 129-175) -- retrieves from MinIO, extracts snippet from fn signal{...} wrapper
signals.json in MinIO
- Write: builder/src/main.rs (lines 704-733) -- build_parquet() creates parquet result, sig_map serialized as signals.json to runs/{run_id}/signals.json
- Read: go/internal/result/handler.go RunResult() (lines 41-92) -- GET /chart/run-result?run_id=... fetches from MinIO bucket at runs/{run_id}/signals.json
- Delete: go/internal/result/handler.go DeleteRun() (lines 94-127) -- removes both result.parquet and signals.json
Run Worker (Go)
- File: go/internal/run/worker.go (lines 1-189)
- Start() (line 23): polls both run_jobs and rl_jobs every 3 seconds
- pollRuns() (line 31): claims pending run jobs, fetches missing candles, marks as ready
- pollRLJobs() (line 56): claims pending RL jobs, fetches candles, marks as training
6. EXECUTION / WASM
Position Sizing
- Policy Instruments: policy-worker/src/main.rs (lines 28-33) -- Instrument struct with quantity field
- Trade Job Creation: record_signal() (lines 477-532) -- inserts into trade_jobs with quantity from policy instrument
- Margin Check: trade-worker/src/main.rs check_margin() (lines 94-127) -- calls Dhan /margincalculator before placing order
- Fund Limit Check: trade-worker/src/main.rs check_funds() (lines 54-76) -- calls Dhan /fundlimit
- Max Trade Value: trade-worker/src/main.rs (lines 248-260) -- max_trade_value from policy, rejects if price * quantity > limit
Deadband Logic
- RL Training: position_deadband in TrainConfig (line 571, default 0.05), used in step_reward() (line 682), evaluate() (line 1544)
- WASM Runtime: builder/src/main.rs SCAFFOLD (line 107): const POSITION_DEADBAND: f64 = 0.05, applied at line 121
- Backtest: builder/src/main.rs compute_metrics() (line 291): const POSITION_DEADBAND: f64 = 0.05 at line 314
EOD Force-Close
- RL Training: train.rs line 831-852, 972-992, 1116-1139 -- day_boundaries check, force-close at end of trading day for intraday intervals
- Backtest: builder/src/main.rs line 319-388 -- checks timestamps[i]/86400 vs timestamps[i+1]/86400 to detect EOD, force-closes position
- Policy Worker: policy-worker/src/main.rs lines 761-795 -- EOD close loop: queries trade_positions for open intraday positions, queues MARKET close orders with correlation_id suffix -eod-close
OHLCV Allocation
- OHLCV Worker: go/internal/ohlcv/worker.go -- creates ohlcv_jobs table entries, processes in 90-day chunks, stores into candles table
- Run Worker Candle Fetch: go/internal/run/worker.go (lines 110-129) -- ensures candles exist before backtest execution
- Buffer Days: builder/src/main.rs (lines 543-549) -- computes warmup buffer from lookback + max_period + velocity_lookback, fetches extra candles before from_date
7. ADMIN PANEL
- Admin Page: web/src/routes/admin/+page.server.ts (lines 1-28)
  - Authorization: checks locals.user.id === OHLCV_USER_ID (line 14)
  - Fetches stats from GET /admin/ohlcv on Go backend (line 18)
  - Generates HMAC WebSocket token for live progress (line 25)
- OHLCV Trigger: web/src/routes/admin/ohlcv/trigger/+server.ts (lines 1-19)
  - POST triggers POST /internal/ohlcv-trigger on Go backend
  - Same OHLCV_USER_ID authorization check (line 5)
- Go Admin Routes: go/internal/ohlcv/handler.go (lines 189-195):
  - GET /admin/ohlcv -- stats endpoint
  - GET /admin/ohlcv/ws -- WebSocket live progress
  - POST /admin/ohlcv/trigger -- admin trigger
  - GET /ohlcv/stocks -- stock list with pagination
  - POST /internal/ohlcv-trigger -- internal trigger (from broker handler notification)
8. INFRASTRUCTURE: rl_jobs QUEUE, RL WORKER/SERVICE
rl_jobs Queue
- Database table: Referenced by rl_jobs across multiple services
- Creation: web/src/routes/strategies/new/rl/+page.server.ts line 39 -- INSERT INTO rl_jobs (strategy_id) VALUES ($1)
- Go Run Worker Polling: go/internal/run/worker.go (lines 56-79) -- pollRLJobs() queries rl_jobs WHERE status = 'pending', fetches candles, updates to status='training'
- Builder RL Processing: builder/src/main.rs (lines 1308-1330) -- polls rl_jobs WHERE status = 'training' (actually pending after Go worker marks them ready), calls process_rl_job()
RL Job Processing Flow
1. UI creates strategy + rl_jobs entry (status: pending)
2. Go run worker (go/internal/run/worker.go pollRLJobs()): fetches required candles, updates status to training
3. Builder (builder/src/main.rs process_rl_job(), line 847): 
   - Reads rl_config from strategy, fetches candle data
   - Computes indicators + stationary transforms + state matrix
   - Trains via train_ppo() or train_reinforce() 
   - Computes feature importance, distills to decision tree
   - Generates Rust code via net_to_rust(), compiles to WASM
   - Uploads weights.bin to MinIO, compiles WASM, updates strategy
   - Stores rl_summary JSON + per-episode rl_training_metrics
   - Updates rl_jobs status to done
Builder Service (main loop)
- File: builder/src/main.rs (lines 1213-1334)
- Polls 3 job types every 3 seconds:
  - build_jobs (line 1228): compiles Rust strategy snippets to WASM
  - run_jobs (line 1279): executes backtests (WASM signals + metrics + Parquet/signals.json to MinIO)
  - rl_jobs (line 1309): full RL training pipeline
- MinIO S3 client for all artifact storage (lines 163-178)
Policy Worker (Live Execution)
- File: policy-worker/src/main.rs (lines 1-908)
- WasmRunner (line 45): loads compiled WASM strategies, maintains candle history buffers, calls run() export
- Daily policies: run_daily_policy() (line 588) -- runs once per poll cycle
- Intraday policies: run_intraday_policies() (line 635) -- connects to Dhan WebSocket feed, processes ticks in real-time, calls WASM tick() per instrument
- Market hours check: is_market_hours() (line 561) -- 9:15 AM to 3:30 PM IST
- Signal recording: record_signal() (line 477) -- inserts into live_signals or trade_jobs based on policy mode (alert vs trade)
- EOD force-close: lines 761-795 -- squares off all open intraday positions at market close
Trade Worker (Order Execution)
- File: trade-worker/src/main.rs (lines 1-460)
- Polls trade_jobs every 3 seconds
- Full execution pipeline: fund limit check -> margin check -> position reversal handling -> order placement -> order status polling
- Terminal statuses: TRADED, PART_TRADED, REJECTED, CANCELLED, EXPIRED
- Records positions in trade_positions table
- Telegram notifications for fill/rejection
Summary Table
Feature	Status	Key Files
Google OAuth	Implemented	web/src/routes/auth/google/, web/src/lib/server/oauth.ts
Dhan OAuth	Implemented	web/src/routes/auth/dhan/, go/internal/broker/
Session Management	Implemented	web/src/lib/server/session.ts, web/src/hooks.server.ts
Nifty500 Sync	Implemented	go/internal/nifty500/sync.go
OHLCV Worker	Implemented	go/internal/ohlcv/worker.go
LTP/OHLC/Quote Proxies	Implemented	go/internal/market/handler.go
WebSocket Live Feed	Implemented	go/internal/live/handler.go
WMA/ATR/Stoch/OBV/CCI	Implemented	rust/indicators/src/
PPO Training	Implemented	builder/src/rl/train.rs:1323
REINFORCE Training	Implemented	builder/src/rl/train.rs:1206
Actor-Critic MLP	Implemented	builder/src/rl/train.rs:14-444
Distillation	Implemented	builder/src/rl/distill.rs
Saved Charts	Implemented	web/src/routes/charts/
Run CRUD	Implemented	web/src/routes/runs/, go/internal/result/
signals.json MinIO	Implemented	builder/src/main.rs:704, go/internal/result/handler.go:70
Strategy Source	Implemented	go/internal/result/handler.go:129
Position Sizing	Implemented	policy-worker/src/main.rs:28, trade-worker/src/main.rs:248
Deadband Logic	Implemented	builder/src/main.rs:107, builder/src/rl/train.rs:571
EOD Force-Close	Implemented	policy-worker/src/main.rs:761, builder/src/main.rs:319
OHLCV Allocation	Implemented	go/internal/ohlcv/worker.go, go/internal/run/worker.go
Admin Routes	Implemented	web/src/routes/admin/, go/internal/ohlcv/handler.go
rl_jobs Queue	Implemented	builder/src/main.rs:1308, go/internal/run/worker.go:56
RL Worker/Service	Implemented	builder/src/main.rs (polls + processes RL jobs)
Policy Worker	Implemented	policy-worker/src/main.rs (WASM + Dhan feed)
Trade Worker	Implemented	trade-worker/src/main.rs (order execution)