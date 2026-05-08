# Backtest Result Chart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a candlestick chart with buy/sell markers on the backtest run detail page, with one tab per instrument.

**Architecture:** The Rust builder writes a compact `signals.json` file to MinIO alongside the existing Parquet. A new Go endpoint reads this file and serves it as JSON after verifying run ownership. The run detail page fetches candles via the existing `/api/candles` endpoint and signals via a new `/api/run-result` endpoint, then renders a lightweight-charts chart with `arrowUp`/`arrowDown` markers.

**Tech Stack:** Rust (serde_json), Go (minio-go/v7), SvelteKit, lightweight-charts

---

### Task 1: Write signals.json in Rust builder

**Files:**
- Modify: `builder/src/main.rs`

The builder already has `upload()`. After uploading the Parquet, write a signals.json with only non-zero signal rows, grouped so the frontend can look up by `"sec:seg"` key.

- [ ] **Step 1: Add serde_json to builder Cargo.toml**

In `builder/Cargo.toml`, add to `[dependencies]`:
```toml
serde_json = "1"
```

- [ ] **Step 2: Build signals JSON and upload in `process_run_job`**

In `builder/src/main.rs`, after the `upload(s3, bucket, &result_key, parquet).await?;` line and before the DB update, add:

```rust
// Build compact signals JSON: {"sec:seg": [{ts, sig}, ...]}
let mut sig_map: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();
for i in 0..col_ts.len() {
    if col_sig[i] == 0 { continue; }
    let key = format!("{}:{}", col_sec[i], col_seg[i]);
    sig_map.entry(key).or_default().push(serde_json::json!({
        "ts": col_ts[i],
        "sig": col_sig[i]
    }));
}
let signals_json = serde_json::to_vec(&sig_map).map_err(|e| e.to_string())?;
let signals_key = format!("runs/{}/signals.json", run_id);
upload(s3, bucket, &signals_key, signals_json).await?;
```

- [ ] **Step 3: Verify builder compiles**

```bash
cd /Users/madhavkandukuri/GitHub/madhav165/dhan-test/builder
cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add builder/Cargo.toml builder/src/main.rs
git commit -m "Write signals.json to MinIO after each backtest run"
```

---

### Task 2: Go result endpoint — MinIO client + handler

**Files:**
- Create: `go/internal/result/handler.go`
- Modify: `go/cmd/server/main.go`

- [ ] **Step 1: Add minio-go dependency**

```bash
cd /Users/madhavkandukuri/GitHub/madhav165/dhan-test/go
go get github.com/minio/minio-go/v7@latest
```

Expected: go.mod and go.sum updated with minio-go.

- [ ] **Step 2: Create `go/internal/result/handler.go`**

```go
package result

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"

	"github.com/minio/minio-go/v7"
	"github.com/minio/minio-go/v7/pkg/credentials"
)

type Handler struct {
	DB     *sql.DB
	bucket string
	s3     *minio.Client
}

func NewHandler(db *sql.DB) (*Handler, error) {
	endpoint := os.Getenv("MINIO_ENDPOINT")
	user := os.Getenv("MINIO_ROOT_USER")
	pass := os.Getenv("MINIO_ROOT_PASSWORD")
	bucket := os.Getenv("MINIO_BUCKET")
	if bucket == "" {
		bucket = "dhan"
	}
	client, err := minio.New(endpoint, &minio.Options{
		Creds:  credentials.NewStaticV4(user, pass, ""),
		Secure: false,
	})
	if err != nil {
		return nil, fmt.Errorf("minio client: %w", err)
	}
	return &Handler{DB: db, bucket: bucket, s3: client}, nil
}

func (h *Handler) RunResult(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	runID := r.URL.Query().Get("run_id")
	if userID == "" || runID == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}

	// Verify ownership and get result_key
	var resultKey sql.NullString
	err := h.DB.QueryRowContext(r.Context(),
		`select r.result_key
		 from backtest_runs r
		 join strategies s on s.id = r.strategy_id
		 where r.id = $1 and s.user_id = $2`,
		runID, userID,
	).Scan(&resultKey)
	if err == sql.ErrNoRows {
		http.Error(w, "not found", http.StatusNotFound)
		return
	}
	if err != nil {
		http.Error(w, "db error", http.StatusInternalServerError)
		return
	}
	if !resultKey.Valid {
		http.Error(w, "result not ready", http.StatusNotFound)
		return
	}

	// Derive signals.json key from result_key: runs/{id}/result.parquet → runs/{id}/signals.json
	sigKey := fmt.Sprintf("runs/%s/signals.json", runID)
	obj, err := h.s3.GetObject(context.Background(), h.bucket, sigKey, minio.GetObjectOptions{})
	if err != nil {
		http.Error(w, "signals not found", http.StatusNotFound)
		return
	}
	defer obj.Close()

	data, err := io.ReadAll(obj)
	if err != nil {
		http.Error(w, "read error", http.StatusInternalServerError)
		return
	}

	// Validate it's JSON before proxying
	var raw json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		http.Error(w, "invalid signals data", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.Write(data)
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /chart/run-result", h.RunResult)
}
```

- [ ] **Step 3: Register in main.go**

In `go/cmd/server/main.go`, add import:
```go
"github.com/madhav165/dhan-test/go/internal/result"
```

After `lh := live.NewHandler(database, key)`, add:
```go
rh, err := result.NewHandler(database)
if err != nil {
    log.Fatalf("result handler: %v", err)
}
```

After `lh.RegisterRoutes(mux)`, add:
```go
rh.RegisterRoutes(mux)
```

- [ ] **Step 4: Verify Go builds**

```bash
cd /Users/madhavkandukuri/GitHub/madhav165/dhan-test/go
go build ./...
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add go/internal/result/handler.go go/cmd/server/main.go go/go.mod go/go.sum
git commit -m "Add /chart/run-result endpoint to serve backtest signals from MinIO"
```

---

### Task 3: SvelteKit proxy for run-result

**Files:**
- Create: `web/src/routes/api/run-result/+server.ts`

- [ ] **Step 1: Create the proxy route**

```bash
mkdir -p /Users/madhavkandukuri/GitHub/madhav165/dhan-test/web/src/routes/api/run-result
```

Create `web/src/routes/api/run-result/+server.ts`:
```typescript
import { GO_URL } from '$env/static/private'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, locals }) => {
	if (!locals.user) return new Response('Unauthorized', { status: 401 })

	const params = url.searchParams.toString()
	const resp = await fetch(`${GO_URL}/chart/run-result?${params}`, {
		headers: { 'X-User-ID': locals.user.id },
	})

	const data = await resp.text()
	return new Response(data, {
		status: resp.status,
		headers: { 'Content-Type': 'application/json' },
	})
}
```

- [ ] **Step 2: Run svelte-kit sync to generate types**

```bash
cd /Users/madhavkandukuri/GitHub/madhav165/dhan-test/web
npx svelte-kit sync
```

Expected: `.svelte-kit/types/src/routes/api/run-result/$types.d.ts` created.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/api/run-result/+server.ts
git commit -m "Add /api/run-result SvelteKit proxy"
```

---

### Task 4: Run detail page — add security_id to instruments query

**Files:**
- Modify: `web/src/routes/runs/[run_id]/+page.server.ts`

The existing query's `json_build_object` is missing `security_id`. The chart needs it to fetch candles.

- [ ] **Step 1: Add security_id to the instruments aggregate**

In `web/src/routes/runs/[run_id]/+page.server.ts`, change:
```typescript
		        array_agg(
		          json_build_object(
		            'trading_symbol', i.trading_symbol,
		            'exchange_segment', ri.exchange_segment
		          ) order by i.trading_symbol
		        ) as instruments
```
to:
```typescript
		        array_agg(
		          json_build_object(
		            'trading_symbol', i.trading_symbol,
		            'security_id', ri.security_id,
		            'exchange_segment', ri.exchange_segment
		          ) order by i.trading_symbol
		        ) as instruments
```

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/runs/[run_id]/+page.server.ts
git commit -m "Include security_id in run instruments for chart fetching"
```

---

### Task 5: Run detail page — chart with tabs and markers

**Files:**
- Modify: `web/src/routes/runs/[run_id]/+page.svelte`

The existing page shows metrics. Add below the metrics section: a tab bar (one tab per instrument) and a lightweight-charts candlestick chart with buy/sell markers.

- [ ] **Step 1: Add chart imports and state to the script block**

At the top of the `<script>` block, after the existing imports, add:
```typescript
import { onMount, onDestroy } from 'svelte'
import { createChart, ColorType, CandlestickSeries, type IChartApi, type ISeriesApi, type CandlestickData } from 'lightweight-charts'

type Instrument = { trading_symbol: string; security_id: string; exchange_segment: string }
type Signal = { ts: number; sig: number }

const IST_OFFSET = 5.5 * 60 * 60
const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
const pad = (n: number) => String(n).padStart(2, '0')
const fmtIST = (ts: number) => {
	const d = new Date((ts + IST_OFFSET) * 1000)
	const h = d.getUTCHours(), m = d.getUTCMinutes()
	return h === 0 && m === 0
		? `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} '${String(d.getUTCFullYear()).slice(2)}`
		: `${pad(h)}:${pad(m)}`
}
const tickFmtIST = (ts: number, type: number) => {
	const d = new Date((ts + IST_OFFSET) * 1000)
	const Y = d.getUTCFullYear(), M = d.getUTCMonth(), D = d.getUTCDate()
	const h = d.getUTCHours(), m = d.getUTCMinutes()
	if (type === 0) return String(Y)
	if (type === 1) return `${MONTHS[M]} '${String(Y).slice(2)}`
	if (type === 2) return `${D} ${MONTHS[M]}`
	return `${pad(h)}:${pad(m)}`
}

const instruments = $derived(
	(run.instruments ?? []).filter((i: Instrument) => i.trading_symbol) as Instrument[]
)
let activeTab = $state(0)

let chartContainer: HTMLDivElement
let chart: IChartApi
let candleSeries: ISeriesApi<'Candlestick'>
let chartError = $state('')
let chartLoading = $state(false)

// signals keyed by "sec:seg"
let allSignals = $state<Record<string, Signal[]>>({})

async function loadSignals() {
	if (!run.result_key) return
	const res = await fetch(`/api/run-result?run_id=${run.id}`)
	if (res.ok) allSignals = await res.json()
}

async function loadChart(inst: Instrument) {
	if (!chart || !inst) return
	chartLoading = true
	chartError = ''
	try {
		const from = run.from_date.slice(0, 10)
		const to = run.to_date.slice(0, 10)
		const res = await fetch(
			`/api/candles?security_id=${inst.security_id}&exchange_segment=${inst.exchange_segment}&interval=${run.interval}&from=${from}&to=${to}`
		)
		if (!res.ok) throw new Error(await res.text())
		const candles: Array<{timestamp: number; open: number; high: number; low: number; close: number}> = await res.json()

		const candleData: CandlestickData[] = candles.map(c => ({
			time: c.timestamp as any,
			open: c.open, high: c.high, low: c.low, close: c.close,
		}))
		candleSeries.setData(candleData)
		chart.timeScale().fitContent()

		// Apply markers
		const key = `${inst.security_id}:${inst.exchange_segment}`
		const sigs = allSignals[key] ?? []
		const markers = sigs
			.filter(s => s.sig === 1 || s.sig === 2)
			.sort((a, b) => a.ts - b.ts)
			.map(s => s.sig === 1
				? { time: s.ts as any, position: 'belowBar' as const, color: '#22c55e', shape: 'arrowUp' as const, text: 'B' }
				: { time: s.ts as any, position: 'aboveBar' as const, color: '#ef4444', shape: 'arrowDown' as const, text: 'S' }
			)
		candleSeries.setMarkers(markers)
	} catch (e: any) {
		chartError = e.message
	} finally {
		chartLoading = false
	}
}

onMount(async () => {
	chart = createChart(chartContainer, {
		layout: { background: { type: ColorType.Solid, color: '#0f0f0f' }, textColor: '#aaa' },
		grid: { vertLines: { color: '#1a1a1a' }, horzLines: { color: '#1a1a1a' } },
		crosshair: { mode: 1 },
		width: chartContainer.clientWidth,
		height: 400,
		timeScale: {
			fixLeftEdge: true, fixRightEdge: true,
			timeVisible: true, secondsVisible: false,
			tickMarkFormatter: tickFmtIST,
		},
		localization: { timeFormatter: fmtIST },
	})
	candleSeries = chart.addSeries(CandlestickSeries, {
		upColor: '#22c55e', downColor: '#ef4444',
		borderUpColor: '#22c55e', borderDownColor: '#ef4444',
		wickUpColor: '#22c55e', wickDownColor: '#ef4444',
	})

	if (run.job_status === 'done') {
		await loadSignals()
		if (instruments.length > 0) await loadChart(instruments[0])
	}
})

onDestroy(() => chart?.remove())

$effect(() => {
	const inst = instruments[activeTab]
	if (chart && inst) loadChart(inst)
})
```

- [ ] **Step 2: Add chart HTML below the metrics section**

After the closing `</section>` tag of the Results section, add:

```svelte
{#if run.job_status === 'done' && instruments.length > 0}
	<section class="chart-section">
		<h2>Chart</h2>

		{#if instruments.length > 1}
			<div class="tab-bar">
				{#each instruments as inst, i}
					<button
						class="tab-btn"
						class:active={activeTab === i}
						onclick={() => { activeTab = i }}
					>
						{inst.trading_symbol}
					</button>
				{/each}
			</div>
		{/if}

		{#if chartError}
			<p class="chart-error">{chartError}</p>
		{/if}

		<div class="chart-wrap" class:loading={chartLoading}>
			<div bind:this={chartContainer} class="chart-canvas"></div>
			{#if chartLoading}
				<div class="chart-overlay">Loading…</div>
			{/if}
		</div>
	</section>
{/if}
```

- [ ] **Step 3: Add CSS for the chart section**

Inside the `<style>` block, append:

```css
.chart-section {
	margin-top: 32px;
}

.tab-bar {
	display: flex;
	gap: 4px;
	margin-bottom: 12px;
}

.tab-btn {
	background: none;
	border: 1px solid var(--border);
	border-radius: 6px;
	color: var(--text-muted);
	cursor: pointer;
	font-family: 'Inter', sans-serif;
	font-size: 0.8rem;
	padding: 5px 12px;
}

.tab-btn.active {
	border-color: var(--accent);
	color: var(--accent);
}

.chart-wrap {
	border-radius: 8px;
	overflow: hidden;
	position: relative;
}

.chart-canvas {
	height: 400px;
	width: 100%;
}

.chart-overlay {
	align-items: center;
	background: rgba(0, 0, 0, 0.5);
	color: var(--text-muted);
	display: flex;
	font-size: 0.875rem;
	inset: 0;
	justify-content: center;
	position: absolute;
}

.chart-error {
	color: var(--red);
	font-size: 0.8rem;
	margin-bottom: 8px;
}
```

- [ ] **Step 4: Check svelte-check for errors on the page**

```bash
cd /Users/madhavkandukuri/GitHub/madhav165/dhan-test/web
npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep "run_id"
```
Expected: no errors for `runs/[run_id]/+page.svelte`.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/runs/\[run_id\]/+page.svelte web/src/routes/runs/\[run_id\]/+page.server.ts
git commit -m "Add backtest result chart with buy/sell markers, tabbed per instrument"
```

---

### Task 6: Wire up docker-compose for Go MinIO access

**Files:**
- Modify: `docker-compose.yml`

The Go service needs MINIO_* env vars when running in Docker.

- [ ] **Step 1: Add MinIO env vars to the go service**

In `docker-compose.yml`, under the `go:` service's `environment:` block, add:
```yaml
      MINIO_ENDPOINT: host.docker.internal:9000
      MINIO_ROOT_USER: ${MINIO_ROOT_USER}
      MINIO_ROOT_PASSWORD: ${MINIO_ROOT_PASSWORD}
      MINIO_BUCKET: ${MINIO_BUCKET:-dhan}
```

- [ ] **Step 2: Commit**

```bash
git add docker-compose.yml
git commit -m "Pass MinIO env vars to Go service for run-result endpoint"
```
