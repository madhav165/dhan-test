<script lang="ts">
	import { invalidateAll } from '$app/navigation'
	import { onDestroy } from 'svelte'
	import { createChart, createSeriesMarkers, ColorType, CandlestickSeries, type IChartApi, type ISeriesApi, type CandlestickData, type ISeriesMarkersPluginApi } from 'lightweight-charts'

	let { data } = $props()
	const run = $derived(data.run)

	const pending = $derived(
		run.job_status === 'pending' || run.job_status === 'ready' || run.job_status === 'running'
	)

	let timer: ReturnType<typeof setInterval>
	$effect(() => {
		if (pending) {
			timer = setInterval(() => invalidateAll(), 3000)
		} else {
			clearInterval(timer)
		}
	})

	onDestroy(() => { clearInterval(timer); chart?.remove() })

	function fmt(v: number | null, decimals = 2) {
		return v != null ? Number(v).toFixed(decimals) : '—'
	}

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

	let chartContainer: HTMLDivElement = $state() as HTMLDivElement
	let chart: IChartApi
	let candleSeries: ISeriesApi<'Candlestick'>
	let markersPlugin: ISeriesMarkersPluginApi<number> | null = null
	let chartError = $state('')
	let chartLoading = $state(false)
	let chartLoaded = $state(false)

	let allSignals = $state<Record<string, Signal[]>>({})

	async function loadSignals() {
		if (!run.result_key) return
		const res = await fetch(`/api/run-result?run_id=${run.id}`)
		if (res.ok) {
			allSignals = await res.json()
		} else {
			chartError = `Signals not available (${res.status}) — re-run the backtest to generate them`
		}
	}

	async function loadChart(inst: Instrument) {
		if (!chart || !inst) return
		chartLoading = true
		chartError = ''
		try {
			const from = new Date(run.from_date).toISOString().slice(0, 10)
			const to = new Date(run.to_date).toISOString().slice(0, 10)
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

			const key = `${inst.security_id}:${inst.exchange_segment}`
			const sigs = (allSignals[key] ?? []).sort((a, b) => a.ts - b.ts)
			// only mark signal transitions, not every candle with the same signal
			const markers: any[] = []
			let prevSig = 0
			for (const s of sigs) {
				if (s.sig !== prevSig && (s.sig === 1 || s.sig === 2)) {
					markers.push(s.sig === 1
						? { time: s.ts as any, position: 'belowBar' as const, color: '#22c55e', shape: 'arrowUp' as const, text: 'B' }
						: { time: s.ts as any, position: 'aboveBar' as const, color: '#ef4444', shape: 'arrowDown' as const, text: 'S' }
					)
				}
				prevSig = s.sig
			}
			if (!markersPlugin) {
				markersPlugin = createSeriesMarkers(candleSeries, markers)
			} else {
				markersPlugin.setMarkers(markers)
			}
		} catch (e: any) {
			chartError = e.message
		} finally {
			chartLoading = false
		}
	}

	$effect(() => {
		if (!chartContainer || chart) return
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
		if (!chartLoaded) {
			chartLoaded = true
			loadSignals().then(() => {
				if (instruments.length > 0) loadChart(instruments[0])
			})
		}
	})
</script>

<div class="header">
	<div>
		<a href="/runs" class="back">← Runs</a>
		<h1>{run.strategy_name}</h1>
		<p class="meta">
			{run.instruments?.filter((i: any) => i.trading_symbol).map((i: any) => i.trading_symbol).join(', ') || '—'}
			· {run.interval} · {new Date(run.from_date).toISOString().slice(0, 10)} → {new Date(run.to_date).toISOString().slice(0, 10)}
		</p>
	</div>
	<span class="badge {run.job_status}">{run.job_status ?? 'unknown'}</span>
</div>

{#if run.job_status === 'failed' && run.job_error}
	<div class="error-box">
		<p class="error-label">Error</p>
		<pre class="error-text">{run.job_error}</pre>
	</div>
{/if}

<section>
	<h2>Results</h2>
	{#if run.job_status === 'done'}
		<div class="metrics">
			<div class="metric">
				<span class="metric-label">Trades</span>
				<span class="metric-value">{run.num_trades ?? '—'}</span>
			</div>
			<div class="metric">
				<span class="metric-label">Total PnL</span>
				<span class="metric-value">{fmt(run.total_pnl)}</span>
			</div>
			<div class="metric">
				<span class="metric-label">Win Rate</span>
				<span class="metric-value">{run.win_rate != null ? (run.win_rate * 100).toFixed(1) + '%' : '—'}</span>
			</div>
			<div class="metric">
				<span class="metric-label">Max Drawdown</span>
				<span class="metric-value">{fmt(run.max_drawdown)}</span>
			</div>
		</div>
	{:else if pending}
		<p class="waiting">Running…</p>
	{:else}
		<p class="waiting">—</p>
	{/if}
</section>

{#if run.job_status === 'done' && instruments.length > 0}
	<section class="chart-section">
		<h2>Chart</h2>

		{#if instruments.length > 1}
			<div class="tab-bar">
				{#each instruments as inst, i}
					<button
						class="tab-btn"
						class:active={activeTab === i}
						onclick={() => { activeTab = i; loadChart(inst) }}
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

<style>
	.header {
		align-items: flex-start;
		display: flex;
		justify-content: space-between;
		margin-bottom: 32px;
	}

	.back {
		color: var(--text-muted);
		font-size: 0.8rem;
		text-decoration: none;
	}

	.back:hover { color: var(--text); }

	h1 {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 8px 0 4px;
	}

	.meta {
		color: var(--text-muted);
		font-size: 0.8rem;
		margin: 0;
	}

	.badge {
		border-radius: 4px;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		padding: 2px 8px;
		text-transform: uppercase;
	}

	.badge.done { background: var(--green); color: #000; }
	.badge.pending, .badge.ready, .badge.running { background: var(--accent); color: #000; }
	.badge.failed { background: var(--red-bg); color: var(--red-muted); }

	.error-box {
		background: var(--red-bg);
		border-radius: 8px;
		margin-bottom: 24px;
		padding: 14px 16px;
	}

	.error-label {
		color: var(--red-muted);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		margin: 0 0 8px;
		text-transform: uppercase;
	}

	.error-text {
		color: var(--red-muted);
		font-size: 0.8rem;
		margin: 0;
		white-space: pre-wrap;
		word-break: break-all;
	}

	section { margin-bottom: 32px; }

	h2 {
		color: var(--text-muted);
		font-size: 0.8rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		margin: 0 0 16px;
		text-transform: uppercase;
	}

	.metrics {
		display: grid;
		gap: 12px;
		grid-template-columns: repeat(4, 1fr);
	}

	.metric {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 16px;
	}

	.metric-label {
		color: var(--text-muted);
		font-size: 0.75rem;
		font-weight: 500;
	}

	.metric-value {
		color: var(--text);
		font-size: 1.25rem;
		font-weight: 600;
	}

	.waiting {
		color: var(--text-muted);
		font-size: 0.875rem;
	}

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
</style>
