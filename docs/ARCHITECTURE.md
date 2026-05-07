# Architecture

## User mental model

There are four things a user works with:

**Broker connection** — You connect your Dhan account once. The platform stores your access token (encrypted) and uses it on your behalf to fetch market data and place orders. Tokens expire daily; you reconnect each session.

**Strategy** — A piece of logic written in Rust that looks at a price series and emits signals: hold (0), buy (1), or sell (2). A strategy has no opinion about which instruments it runs on or what dates — it is purely the rule. You write it in the editor and the platform compiles it to WebAssembly.

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
                                │
                                └──▶  Postgres / MinIO

Builder (Rust)  ──▶  Postgres  (polls job queues)
                └──▶  MinIO    (reads WASM, writes parquet)
```

**SvelteKit** handles all UI and server-side data loading. Forms submit directly to `+page.server.ts` actions which write to Postgres. There is no REST API between the frontend and the database — SvelteKit talks to Postgres directly via the `pg` client.

**Go service** does two things: proxies authenticated Dhan API calls from the browser (live market data), and runs the candle worker that fetches and stores historical data before a backtest. It holds the encryption key for broker tokens and is the only service that talks to the Dhan API directly.

**Builder** is a Rust binary that runs as a background container. It polls two job queues in Postgres and processes them one at a time:
- `build_jobs` — compiles a strategy's Rust source to WebAssembly and stores the `.wasm` in MinIO
- `run_jobs` — executes a backtest once candle data is ready

### Job queues

Both queues are plain Postgres tables polled every 3 seconds. No message broker.

`build_jobs` status flow: `pending → building → done / failed`

`run_jobs` status flow: `pending → ready → running → done / failed`

The Go candle worker owns the `pending → ready` transition. It checks whether the candles table already has data for the requested instruments, interval, and date range. If not, it calls Dhan's historical API and upserts the rows. Once all instruments are covered it marks the job `ready`.

The builder then picks up `ready` jobs, reads candles from Postgres, loads the compiled WASM from MinIO, and executes it via wasmtime.

### Strategy compilation

When you save a strategy, the source snippet is stored in Postgres. A `build_jobs` row is inserted. The builder wraps your snippet in a scaffold that provides the `alloc` and `run` exports expected by the runtime, adds the `indicators` crate as a dependency, and compiles with `cargo build --target wasm32-unknown-unknown`. The resulting `.wasm` is uploaded to MinIO and the strategy record is updated with its key.

The indicators crate (`rust/indicators`) provides RSI, SMA, and EMA. It is compiled into every strategy WASM — there are no dynamic calls between modules.

### Backtest execution

The WASM interface is two functions:
- `alloc(len: u32) -> *mut f64` — allocates a price buffer inside WASM memory and returns a pointer
- `run(len: u32) -> *mut u8` — runs the signal logic over the prices and returns a pointer to the signal buffer

The builder writes close prices into WASM memory through `alloc`, calls `run`, and reads back the signal bytes. Signals are 0 (hold), 1 (buy), 2 (sell).

Metrics are computed from the signal series: each buy opens a position at that bar's close, the next sell closes it. PnL, win rate, and max drawdown are calculated across all instruments combined.

Results are written as a Parquet file to MinIO (`runs/{run_id}/result.parquet`) with columns: `security_id`, `exchange_segment`, `timestamp`, `open`, `high`, `low`, `close`, `volume`, `signal`. Summary metrics are written back to the `backtest_runs` row.

### Market data

Historical candles are stored in a shared `candles` table keyed on `(security_id, exchange_segment, interval, timestamp)`. If two runs request the same instrument and date range, the data is fetched once and reused. Intraday data from Dhan has a 90-day per-request limit; the Go worker paginates automatically.

The instrument master (235k rows covering NSE equities, BSE equities, and NSE indices) is synced from Dhan's scrip master CSV at 9 AM IST daily. The UI search queries this table with a prefix-first ordering.

### Storage layout in MinIO

```
strategies/{strategy_id}/source.rs      — full scaffolded Rust source
strategies/{strategy_id}/strategy.wasm  — compiled WebAssembly
runs/{run_id}/result.parquet            — per-candle signals and OHLCV
```

### Encryption

Dhan access tokens are encrypted with AES-256-GCM before storage. The key is a 32-byte value passed as a hex string via `ENCRYPTION_KEY`. The nonce is prepended to the ciphertext and the whole thing is base64-encoded. Only the Go service decrypts tokens.
