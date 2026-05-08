<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
	import { enhance } from '$app/forms'
	import { goto } from '$app/navigation'
	import { createChart, ColorType, CandlestickSeries, LineSeries, type IChartApi, type ISeriesApi, type CandlestickData, type LineData } from 'lightweight-charts'
	import InstrumentSearch from '$lib/components/InstrumentSearch.svelte'
	import { loadIndicators, runIndicator } from '$lib/wasm/indicators'

	let { data, form } = $props()

	type Instrument = { security_id: string; exchange_segment: string; trading_symbol: string; custom_symbol: string }
	type Indicator = { type: 'sma' | 'ema' | 'rsi'; period: number }
	type Candle = { timestamp: number; open: number; high: number; low: number; close: number; volume: number }

	// active chart state
	const first = data.charts[0]
	let chartId = $state<string | null>(first?.id ?? null)
	let chartName = $state(first?.name ?? 'New chart')
	let instrument = $state<Instrument | null>(
		first ? {
			security_id: first.security_id,
			exchange_segment: first.exchange_segment,
			trading_symbol: first.name,
			custom_symbol: first.name
		} : null
	)
	let interval = $state(first?.interval ?? 'day')
	let indicators = $state<Indicator[]>(first?.indicators ?? [])

	// indicator form
	let newIndicatorType = $state<'sma' | 'ema' | 'rsi'>('sma')
	let newIndicatorPeriod = $state(14)

	// chart DOM
	let chartContainer: HTMLDivElement
	let chart: IChartApi
	let candleSeries: ISeriesApi<'Candlestick'>
	let lineSeries: ISeriesApi<'Line'>[] = []
	let rsiSeries: ISeriesApi<'Line'>[] = []

	let candles = $state<Candle[]>([])
	let loading = $state(false)
	let error = $state('')

	const GO_BASE = 'http://localhost:8080'
	const INTERVALS = ['1min', '5min', '15min', '25min', '60min', 'day']

	onMount(async () => {
		chart = createChart(chartContainer, {
			layout: { background: { type: ColorType.Solid, color: '#0f0f0f' }, textColor: '#aaa' },
			grid: { vertLines: { color: '#1a1a1a' }, horzLines: { color: '#1a1a1a' } },
			crosshair: { mode: 1 },
			width: chartContainer.clientWidth,
			height: chartContainer.clientHeight,
		})
		candleSeries = chart.addSeries(CandlestickSeries, {
			upColor: '#22c55e', downColor: '#ef4444',
			borderUpColor: '#22c55e', borderDownColor: '#ef4444',
			wickUpColor: '#22c55e', wickDownColor: '#ef4444',
		})

		if (instrument) await fetchCandles()
		chartReady = true
	})

	onDestroy(() => { chart?.remove() })

	async function fetchCandles() {
		if (!instrument) return
		loading = true
		error = ''

		const to = new Date().toISOString().slice(0, 10)
		const from = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10)

		try {
			const res = await fetch(
				`${GO_BASE}/chart/candles?security_id=${instrument.security_id}&exchange_segment=${instrument.exchange_segment}&interval=${interval}&from=${from}&to=${to}`
			)
			if (!res.ok) throw new Error(await res.text())
			candles = await res.json()
			renderCandles()
			await renderIndicators()
		} catch (e: any) {
			error = e.message
		} finally {
			loading = false
		}
	}

	function renderCandles() {
		const data: CandlestickData[] = candles.map(c => ({
			time: c.timestamp as any,
			open: c.open, high: c.high, low: c.low, close: c.close
		}))
		candleSeries.setData(data)
		chart.timeScale().fitContent()
	}

	async function renderIndicators() {
		// remove existing overlays
		lineSeries.forEach(s => chart.removeSeries(s))
		rsiSeries.forEach(s => chart.removeSeries(s))
		lineSeries = []
		rsiSeries = []

		if (candles.length === 0 || indicators.length === 0) return

		const wasm = await loadIndicators()
		const closes = candles.map(c => c.close)
		const times = candles.map(c => c.timestamp)

		const colors = ['#f59e0b', '#818cf8', '#34d399', '#f472b6', '#60a5fa']
		let colorIdx = 0

		for (const ind of indicators) {
			const fn = ind.type === 'sma' ? wasm.sma_run : ind.type === 'ema' ? wasm.ema_run : wasm.rsi_run
			const values = runIndicator(wasm, fn, closes, ind.period)

			const lineData: LineData[] = values
				.map((v, i) => ({ time: times[i] as any, value: v }))
				.filter(p => !isNaN(p.value))

			if (ind.type === 'rsi') {
				const s = chart.addSeries(LineSeries, { color: colors[colorIdx++ % colors.length], lineWidth: 1, priceScaleId: 'rsi' })
				s.setData(lineData)
				rsiSeries.push(s)
			} else {
				const s = chart.addSeries(LineSeries, { color: colors[colorIdx++ % colors.length], lineWidth: 1 })
				s.setData(lineData)
				lineSeries.push(s)
			}
		}
	}

	function selectChart(c: typeof data.charts[0]) {
		chartId = c.id
		chartName = c.name
		instrument = { security_id: c.security_id, exchange_segment: c.exchange_segment, trading_symbol: c.name, custom_symbol: c.name }
		interval = c.interval
		indicators = c.indicators
		fetchCandles()
	}

	function newChart() {
		chartId = null
		chartName = 'New chart'
		instrument = null
		interval = 'day'
		indicators = []
		candles = []
		candleSeries?.setData([])
	}

	function selectInstrument(inst: Instrument) {
		instrument = inst
		if (!chartName || chartName === 'New chart') chartName = inst.trading_symbol
		fetchCandles()
	}

	function addIndicator() {
		if (newIndicatorPeriod < 1) return
		indicators = [...indicators, { type: newIndicatorType, period: newIndicatorPeriod }]
		renderIndicators()
	}

	function removeIndicator(i: number) {
		indicators = indicators.filter((_, idx) => idx !== i)
		renderIndicators()
	}

	let chartReady = false
	$effect(() => {
		interval // track interval
		if (chartReady && instrument) fetchCandles()
	})

	function designStrategy() {
		const params = new URLSearchParams()
		if (chartId) params.set('from_chart', chartId)
		if (instrument) {
			params.set('indicator_types', indicators.map(i => i.type).join(','))
			params.set('indicator_periods', indicators.map(i => i.period).join(','))
		}
		goto(`/strategies/new?${params}`)
	}
</script>

<div class="layout">
	<div class="sidebar">
		<div class="sidebar-header">
			<span class="label">Charts</span>
			<button class="icon-btn" onclick={newChart} title="New chart">+</button>
		</div>
		<ul class="chart-list">
			{#each data.charts as c}
				<li class="chart-item" class:active={c.id === chartId}>
					<button onclick={() => selectChart(c)} class="chart-btn">{c.name}</button>
				</li>
			{/each}
		</ul>
	</div>

	<div class="main">
		<div class="toolbar">
			<InstrumentSearch onselect={selectInstrument} inputId="chart-instrument" placeholder={instrument?.trading_symbol ?? 'Search instrument…'} />

			<select bind:value={interval} class="interval-select">
				{#each INTERVALS as iv}
					<option value={iv}>{iv}</option>
				{/each}
			</select>

			<span class="spacer"></span>

			{#if instrument}
				<form method="POST" action="?/save" use:enhance={({ formData }) => {
					formData.set('indicators', JSON.stringify(indicators))
					if (chartId) formData.set('id', chartId)
					return async ({ result, update }) => {
						if (result.type === 'success' && result.data?.id) chartId = result.data.id as string
						await update()
					}
				}}>
					<input type="hidden" name="name" value={chartName} />
					<input type="hidden" name="security_id" value={instrument.security_id} />
					<input type="hidden" name="exchange_segment" value={instrument.exchange_segment} />
					<input type="hidden" name="interval" value={interval} />
					<button type="submit" class="btn-secondary">Save chart</button>
				</form>

				<button onclick={designStrategy} class="btn-primary">Design strategy</button>
			{/if}
		</div>

		{#if error}
			<p class="error">{error}</p>
		{/if}

		<div class="chart-wrap">
			<div bind:this={chartContainer} class="chart" class:loading></div>
			{#if loading}
				<div class="overlay">Loading…</div>
			{/if}
		</div>
	</div>

	<div class="panel">
		<p class="label">Indicators</p>

		<div class="add-indicator">
			<select bind:value={newIndicatorType} class="ind-select">
				<option value="sma">SMA</option>
				<option value="ema">EMA</option>
				<option value="rsi">RSI</option>
			</select>
			<input type="number" bind:value={newIndicatorPeriod} min="1" max="200" class="period-input" />
			<button onclick={addIndicator} class="btn-add">Add</button>
		</div>

		<ul class="ind-list">
			{#each indicators as ind, i}
				<li class="ind-item">
					<span>{ind.type.toUpperCase()} {ind.period}</span>
					<button onclick={() => removeIndicator(i)} class="remove-btn">×</button>
				</li>
			{/each}
		</ul>
	</div>
</div>

<style>
	.layout {
		display: flex;
		height: calc(100vh - 56px);
		overflow: hidden;
	}

	.sidebar {
		background: var(--bg-surface);
		border-right: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		overflow-y: auto;
		width: 160px;
	}

	.sidebar-header {
		align-items: center;
		display: flex;
		justify-content: space-between;
		padding: 12px 12px 8px;
	}

	.label {
		color: var(--text-faint);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.icon-btn {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 1.1rem;
		line-height: 1;
		padding: 0 4px;
	}

	.icon-btn:hover { color: var(--text); }

	.chart-list { list-style: none; margin: 0; padding: 0 8px; }

	.chart-item { border-radius: 6px; margin-bottom: 2px; }
	.chart-item.active { background: var(--bg); }

	.chart-btn {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		padding: 7px 8px;
		text-align: left;
		width: 100%;
	}

	.chart-item.active .chart-btn { color: var(--text); font-weight: 500; }
	.chart-btn:hover { color: var(--text); }

	.main {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
	}

	.toolbar {
		align-items: center;
		border-bottom: 1px solid var(--border);
		display: flex;
		gap: 8px;
		padding: 8px 12px;
	}

	.spacer { flex: 1; }

	.interval-select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		outline: none;
		padding: 5px 8px;
	}

	.chart-wrap {
		flex: 1;
		overflow: hidden;
		position: relative;
	}

	.chart {
		height: 100%;
		width: 100%;
	}

	.overlay {
		align-items: center;
		background: rgba(0,0,0,0.5);
		color: var(--text-muted);
		display: flex;
		font-size: 0.875rem;
		inset: 0;
		justify-content: center;
		position: absolute;
	}

	.error {
		color: var(--red);
		font-size: 0.8rem;
		padding: 8px 12px;
	}

	.panel {
		background: var(--bg-surface);
		border-left: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		gap: 12px;
		overflow-y: auto;
		padding: 16px 12px;
		width: 180px;
	}

	.add-indicator {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.ind-select, .period-input {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		outline: none;
		padding: 5px 8px;
		width: 100%;
	}

	.btn-add {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		padding: 5px 8px;
	}

	.btn-add:hover { border-color: var(--accent); color: var(--accent); }

	.ind-list { list-style: none; margin: 0; padding: 0; }

	.ind-item {
		align-items: center;
		border-bottom: 1px solid var(--border);
		display: flex;
		font-size: 0.8rem;
		justify-content: space-between;
		padding: 6px 0;
	}

	.ind-item:last-child { border-bottom: none; }

	.remove-btn {
		background: none;
		border: none;
		color: var(--text-faint);
		cursor: pointer;
		font-size: 1rem;
		line-height: 1;
		padding: 0 2px;
	}

	.remove-btn:hover { color: var(--red); }

	.btn-primary {
		background: var(--accent);
		border: none;
		border-radius: 6px;
		color: #000;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		font-weight: 500;
		padding: 6px 12px;
		white-space: nowrap;
	}

	.btn-primary:hover { background: var(--accent-hover); }

	.btn-secondary {
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		padding: 6px 12px;
		white-space: nowrap;
	}

	.btn-secondary:hover { color: var(--text); }
</style>
