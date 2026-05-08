# Chart Page Design

## Overview

The chart page is the primary entry point for logged-in users. It lets users view candlestick charts for any instrument, add technical indicators, and save chart configurations. From a chart, users can initiate strategy creation pre-populated with their chosen indicators.

---

## Data Model

### `charts` table

```sql
create table charts (
  id            uuid primary key default gen_random_uuid(),
  user_id       uuid references users(id) on delete cascade,
  name          text not null,
  security_id   bigint not null,
  exchange_segment text not null,
  interval      text not null,
  indicators    jsonb not null default '[]',
  created_at    timestamptz default now(),
  updated_at    timestamptz default now()
);
```

`indicators` is a JSONB array of objects, each with at minimum `type` and `period`:

```json
[
  {"type": "sma", "period": 14},
  {"type": "rsi", "period": 7},
  {"type": "ema", "period": 20}
]
```

Additional params (e.g. `stddev` for Bollinger Bands) can be added to any entry without schema changes.

The most recently updated chart is loaded by default on login.

---

## Indicators WASM

`rust/indicators` is compiled to a standalone `indicators.wasm` with a generic call-time interface:

```rust
// exported functions
fn sma(ptr: *const f64, len: u32, period: u32) -> *mut f64
fn ema(ptr: *const f64, len: u32, period: u32) -> *mut f64
fn rsi(ptr: *const f64, len: u32, period: u32) -> *mut f64
fn alloc(len: u32) -> *mut f64
fn dealloc(ptr: *mut f64, len: u32)
```

Parameters are passed at call time — not baked into the binary. The same `indicators.wasm` handles `sma(closes, 14)` and `sma(closes, 7)` without recompilation.

The WASM binary is compiled once and served as a static asset alongside the SvelteKit app (`/indicators.wasm`). No build jobs, no MinIO.

**Browser data flow:**
1. Fetch candle OHLCV from Go service
2. Instantiate `indicators.wasm` once per page load
3. For each indicator in the chart config, write closes into WASM memory via `alloc`, call the indicator function, read result back as `Float64Array`
4. Pass result series to the charting library as an overlay line

---

## Chart Page Layout

**Route:** `/charts` — default redirect after login (replaces current default landing page)

**Sidebar:** Charts tab added, positioned first in the nav order.

### Components

**Top bar**
- Saved chart selector (dropdown of user's charts by name, most recent first)
- "New chart" button
- Instrument search (same prefix-search component used in runs/policies new forms)
- Interval selector (1min, 5min, 15min, day)

**Main area**
- Candlestick chart (OHLCV) rendered with Lightweight Charts (TradingView)
- Indicator overlays rendered as additional line series on the same chart
- Oscillator indicators (RSI) rendered in a separate sub-pane below

**Right panel**
- List of active indicators with type and period
- "Add indicator" — type dropdown (SMA, EMA, RSI) + period input + Add button
- Remove button per indicator
- Changing params re-runs the WASM and re-renders immediately (no save needed for preview)

**Bottom bar**
- "Save chart" button — upserts the chart row (name, instrument, interval, indicators)
- "Design strategy" button — navigates to `/strategies/new?from_chart={chart_id}` — the strategy new page reads the chart's indicators and pre-populates the editor scaffold

---

## Market Data

**Historical (daily and intraday, static):**
- Browser requests candles via Go service (`GET /api/candles?security_id=&exchange_segment=&interval=&from=&to=`)
- Go checks the `candles` table first; if missing, fetches from Dhan REST API and upserts
- Returns OHLCV array as JSON

**Live (intraday intervals only):**
- After historical data loads, browser opens a WebSocket to the Go service
- Go proxies the Dhan WebSocket feed for the subscribed instrument
- Each tick appends to or updates the last candle on the chart
- On interval change to `day`, WebSocket is closed

---

## Charting Library

**Lightweight Charts (TradingView)** — MIT licensed, purpose-built for financial charts. Supports candlestick series, multiple line series overlays, and sub-panes for oscillators out of the box. Installed as an npm package.

---

## "Design Strategy" Flow

When the user clicks "Design strategy":
- Navigate to `/strategies/new?from_chart={chart_id}`
- The strategy new page fetches the chart record and reads its `indicators` JSONB
- The Rust editor scaffold is pre-populated with `use indicators::{sma, rsi, ema};` imports and a stub `run` function that calls each indicator with its saved period
- User fills in the signal logic and saves — normal build job flow from there

---

## Migration

```sql
-- migration 008
create table charts (
  id               uuid primary key default gen_random_uuid(),
  user_id          uuid references users(id) on delete cascade,
  name             text not null,
  security_id      bigint not null,
  exchange_segment text not null,
  interval         text not null,
  indicators       jsonb not null default '[]',
  created_at       timestamptz default now(),
  updated_at       timestamptz default now()
);
create index idx_charts_user_id on charts (user_id, updated_at desc);
```

---

## Out of Scope

- Drawing tools on the chart
- Multiple chart layouts / split view
- Indicator types beyond SMA, EMA, RSI in the first version
- Chart sharing between users
