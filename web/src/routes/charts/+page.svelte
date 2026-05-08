<script lang="ts">
	import { onMount, onDestroy, untrack } from 'svelte'
	import { enhance } from '$app/forms'
	import { goto, invalidateAll } from '$app/navigation'
	import { createChart, ColorType, CandlestickSeries, LineSeries, HistogramSeries, type IChartApi, type ISeriesApi, type CandlestickData, type LineData, type HistogramData } from 'lightweight-charts'
	import InstrumentSearch from '$lib/components/InstrumentSearch.svelte'
	import { loadIndicators, runIndicator } from '$lib/wasm/indicators'

	let { data } = $props()

	type Instrument = { security_id: string; exchange_segment: string; trading_symbol: string; custom_symbol: string }
	type Indicator = { type: 'sma' | 'ema' | 'rsi'; period: number }
	type Candle = { timestamp: number; open: number; high: number; low: number; close: number; volume: number }

	const first = untrack(() => data.charts[0])
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

	function focus(el: HTMLElement) { el.focus() }

	let menuOpenId = $state<string | null>(null)

	function toggleMenu(id: string, e: MouseEvent) {
		e.stopPropagation()
		menuOpenId = menuOpenId === id ? null : id
	}

	function closeMenu() { menuOpenId = null }

	// sidebar inline rename
	let editingChartId = $state<string | null>(null)
	let editingName = $state('')

	function startRename(c: typeof data.charts[0]) {
		editingChartId = c.id
		editingName = c.name
	}

	let renameFormEl: HTMLFormElement
	let renameIdInput: HTMLInputElement
	let renameNameInput: HTMLInputElement
	let renameSecIdInput: HTMLInputElement
	let renameSegInput: HTMLInputElement
	let renameIntervalInput: HTMLInputElement
	let renameIndicatorsInput: HTMLInputElement

	function commitRename(c: typeof data.charts[0]) {
		const name = editingName.trim()
		editingChartId = null
		if (!name || name === c.name) return
		renameIdInput.value = c.id
		renameNameInput.value = name
		renameSecIdInput.value = c.security_id
		renameSegInput.value = c.exchange_segment
		renameIntervalInput.value = c.interval
		renameIndicatorsInput.value = JSON.stringify(c.indicators)
		renameFormEl.requestSubmit()
	}

	// indicator form
	let newIndicatorType = $state<'sma' | 'ema' | 'rsi'>('sma')
	let newIndicatorPeriod = $state(14)

	// chart DOM
	let chartContainer: HTMLDivElement
	let chart: IChartApi
	let candleSeries: ISeriesApi<'Candlestick'>
	let volumeSeries: ISeriesApi<'Histogram'>
	let lineSeries: ISeriesApi<'Line'>[] = []
	let rsiSeries: ISeriesApi<'Line'>[] = []

	let candles = $state<Candle[]>([])
	let loading = $state(false)
	let error = $state('')
	let settingsOpen = $state(false)
	let isMobile = $state(false)
	let hideVolume = $state(false)
	let mobileView = $state<'candles' | 'volume'>('candles')

	function updateMobileState() {
		const mobile = window.matchMedia('(max-width: 768px)').matches
		const portrait = window.matchMedia('(orientation: portrait)').matches
		isMobile = mobile
		hideVolume = mobile && portrait
	}

	onMount(() => {
		updateMobileState()
		window.addEventListener('resize', updateMobileState)
	})

	const INTERVALS = ['1min', '5min', '15min', '25min', '60min', 'day']

	function defaultDates(iv: string): { from: string; to: string } {
		const days = { '1min': 7, '5min': 14, '15min': 30, '25min': 30, '60min': 30, 'day': 365 }[iv] ?? 30
		const to = new Date().toISOString().slice(0, 10)
		const from = new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString().slice(0, 10)
		return { from, to }
	}

	let { from: initFrom, to: initTo } = defaultDates(untrack(() => interval))
	let fromDate = $state(initFrom)
	let toDate = $state(initTo)

	// scrollbar state: 0 = leftmost, 1 = rightmost
	let scrollPos = $state(0)
	let scrollMax = $state(0)
	let scrollThumb = $state(100)
	let suppressScroll = false

	// live mode
	let live = $state(false)
	let liveWs: WebSocket | null = null
	let formingTime = 0      // UTC epoch seconds of current interval bucket
	let formingOpen = 0
	let formingHigh = 0
	let formingLow = 0
	let formingClose = 0
	let prevVolume = 0       // cumulative volume from last tick (for delta calc)

	const INTERVAL_SECONDS: Record<string, number> = {
		'1min': 60, '5min': 300, '15min': 900, '25min': 1500, '60min': 3600
	}

	const IS_INTRADAY = $derived(interval !== 'day')

	async function startLive() {
		if (!instrument || !IS_INTRADAY) return
		stopLive()
		const ws = new WebSocket(`${data.goWsUrl}/chart/live?token=${data.wsToken}&security_id=${instrument.security_id}&exchange_segment=${instrument.exchange_segment}`)
		liveWs = ws
		ws.onmessage = ({ data: raw }) => {
			const tick: { ltp: number; ltt: number; vol: number } = JSON.parse(raw)
			if (tick.ltt === 0) return
			const ivSec = INTERVAL_SECONDS[interval]
			const bucketTime = Math.floor(tick.ltt / ivSec) * ivSec
			if (bucketTime !== formingTime) {
				formingTime = bucketTime
				formingOpen = tick.ltp
				formingHigh = tick.ltp
				formingLow = tick.ltp
				prevVolume = tick.vol
			} else {
				if (tick.ltp > formingHigh) formingHigh = tick.ltp
				if (tick.ltp < formingLow) formingLow = tick.ltp
			}
			formingClose = tick.ltp
			const deltaVol = Math.max(0, tick.vol - prevVolume)
			prevVolume = tick.vol
			candleSeries.update({
				time: bucketTime as any,
				open: formingOpen,
				high: formingHigh,
				low: formingLow,
				close: formingClose,
			})
			volumeSeries?.update({
				time: bucketTime as any,
				value: deltaVol,
				color: formingClose >= formingOpen ? '#22c55e44' : '#ef444444',
			})
			chart.timeScale().scrollToRealTime()
		}
		ws.onerror = () => { error = 'Live feed error'; live = false }
		ws.onclose = () => { if (live) { live = false } }
	}

	function stopLive() {
		if (liveWs) { liveWs.close(); liveWs = null }
		formingTime = 0
		prevVolume = 0
	}

	$effect(() => {
		if (live) {
			untrack(() => startLive())
		} else {
			stopLive()
		}
	})

	onMount(async () => {
		const IST_OFFSET = 5.5 * 60 * 60 // +5:30 in seconds
		const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
		const istDate = (ts: number) => new Date((ts + IST_OFFSET) * 1000)
		const pad = (n: number) => String(n).padStart(2, '0')

		const fmtIST = (ts: number) => {
			const d = istDate(ts)
			const h = d.getUTCHours(), m = d.getUTCMinutes()
			return h === 0 && m === 0
				? `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} '${String(d.getUTCFullYear()).slice(2)}`
				: `${pad(h)}:${pad(m)}`
		}

		const tickFmtIST = (ts: number, type: number) => {
			const d = istDate(ts)
			const Y = d.getUTCFullYear(), M = d.getUTCMonth(), D = d.getUTCDate()
			const h = d.getUTCHours(), m = d.getUTCMinutes()
			// TickMarkType: 0=Year 1=Month 2=DayOfMonth 3=Time 4=TimeWithSeconds
			if (type === 0) return String(Y)
			if (type === 1) return `${MONTHS[M]} '${String(Y).slice(2)}`
			if (type === 2) return `${D} ${MONTHS[M]}`
			return `${pad(h)}:${pad(m)}`
		}

		chart = createChart(chartContainer, {
			layout: { background: { type: ColorType.Solid, color: '#0f0f0f' }, textColor: '#aaa' },
			grid: { vertLines: { color: '#1a1a1a' }, horzLines: { color: '#1a1a1a' } },
			crosshair: { mode: 1 },
			width: chartContainer.clientWidth,
			height: chartContainer.clientHeight,
			timeScale: { fixLeftEdge: true, fixRightEdge: true, timeVisible: true, secondsVisible: false, tickMarkFormatter: tickFmtIST },
			localization: { timeFormatter: fmtIST },
		})
		candleSeries = chart.addSeries(CandlestickSeries, {
			upColor: '#22c55e', downColor: '#ef4444',
			borderUpColor: '#22c55e', borderDownColor: '#ef4444',
			wickUpColor: '#22c55e', wickDownColor: '#ef4444',
		})
		// on desktop always show volume; on mobile the toggle effect handles it
		if (!window.matchMedia('(max-width: 768px)').matches) {
			volumeSeries = chart.addSeries(HistogramSeries, {
				priceFormat: { type: 'volume' },
				priceScaleId: 'vol',
			}, 1)
			volumeSeries.priceScale().applyOptions({ scaleMargins: { top: 0.7, bottom: 0 } })
		}

		if (instrument) await fetchCandles()
		chartReady = true
	})

	onDestroy(() => {
		chart?.remove()
		window.removeEventListener('resize', updateMobileState)
	})


	// Switch between candles-only and volume-only on mobile
	$effect(() => {
		if (!chart || !chartReady || !isMobile) return
		if (mobileView === 'volume') {
			candleSeries?.applyOptions({ visible: false })
			if (!volumeSeries) {
				volumeSeries = chart.addSeries(HistogramSeries, { priceFormat: { type: 'volume' }, priceScaleId: 'vol' }, 1)
				if (candles.length > 0) volumeSeries.setData(candles.map(c => ({
					time: c.timestamp as any, value: c.volume,
					color: c.close >= c.open ? '#22c55e88' : '#ef444488',
				})))
			}
			volumeSeries.applyOptions({ visible: true })
			volumeSeries.priceScale().applyOptions({ scaleMargins: { top: 0.05, bottom: 0 } })
		} else {
			candleSeries?.applyOptions({ visible: true })
			if (volumeSeries) {
				chart.removeSeries(volumeSeries)
				volumeSeries = undefined as any
			}
		}
	})

	async function fetchCandles(from = fromDate, to = toDate) {
		if (!instrument) return
		loading = true
		error = ''

		try {
			const res = await fetch(
				`/api/candles?security_id=${instrument.security_id}&exchange_segment=${instrument.exchange_segment}&interval=${interval}&from=${from}&to=${to}`
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
		const candleData: CandlestickData[] = candles.map(c => ({
			time: c.timestamp as any,
			open: c.open, high: c.high, low: c.low, close: c.close
		}))
		candleSeries.setData(candleData)

		const volData: HistogramData[] = candles.map(c => ({
			time: c.timestamp as any,
			value: c.volume,
			color: c.close >= c.open ? '#22c55e44' : '#ef444444',
		}))
		volumeSeries?.setData(volData)

		const total = candles.length
		const visible = Math.min(100, total)
		chart.timeScale().setVisibleLogicalRange({ from: total - visible, to: total - 1 })

		chart.timeScale().unsubscribeVisibleLogicalRangeChange(onRangeChange)
		chart.timeScale().subscribeVisibleLogicalRangeChange(onRangeChange)
	}

	function onRangeChange(range: { from: number; to: number } | null) {
		if (!range || suppressScroll) return
		const total = candles.length
		const visibleBars = range.to - range.from
		const maxFrom = total - 1 - visibleBars
		scrollMax = Math.max(0, Math.round(maxFrom))
		scrollThumb = total > 0 ? Math.round((visibleBars / total) * 100) : 100
		suppressScroll = true
		scrollPos = Math.max(0, Math.min(scrollMax, Math.round(range.from)))
		suppressScroll = false
	}

	function onScrollbarInput(e: Event) {
		if (suppressScroll) return
		const val = Number((e.target as HTMLInputElement).value)
		const range = chart.timeScale().getVisibleLogicalRange()
		if (!range) return
		const visibleBars = range.to - range.from
		suppressScroll = true
		chart.timeScale().setVisibleLogicalRange({ from: val, to: val + visibleBars })
		suppressScroll = false
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
		const tradingSymbol = c.name.replace(` · ${c.interval}`, '').trim()
		instrument = { security_id: c.security_id, exchange_segment: c.exchange_segment, trading_symbol: tradingSymbol, custom_symbol: tradingSymbol }
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
		if (chart && candleSeries && volumeSeries) {
			chart.removeSeries(candleSeries)
			chart.removeSeries(volumeSeries)
			candleSeries = chart.addSeries(CandlestickSeries, {
				upColor: '#22c55e', downColor: '#ef4444',
				borderUpColor: '#22c55e', borderDownColor: '#ef4444',
				wickUpColor: '#22c55e', wickDownColor: '#ef4444',
			})
			if (!window.matchMedia('(max-width: 768px)').matches) {
				volumeSeries = chart.addSeries(HistogramSeries, {
					priceFormat: { type: 'volume' },
					priceScaleId: 'vol',
				}, 1)
				volumeSeries.priceScale().applyOptions({ scaleMargins: { top: 0.7, bottom: 0 } })
			}
		}
	}

	function selectInstrument(inst: Instrument) {
		live = false
		instrument = inst
		chartName = `${inst.trading_symbol} · ${interval}`
		fetchCandles(fromDate, toDate)
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

	let chartReady = $state(false)

	$effect(() => {
		if (!chartReady) return
		const iv = interval
		const inst = instrument
		untrack(() => {
			const dates = defaultDates(iv)
			fromDate = dates.from
			toDate = dates.to
			if (!inst) return
			chartName = `${inst.trading_symbol} · ${iv}`
			fetchCandles(dates.from, dates.to)
		})
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

<svelte:window onclick={closeMenu} />

<!-- hidden form for rename — submitted programmatically -->
<form
	bind:this={renameFormEl}
	method="POST"
	action="?/save"
	style="display:none"
	use:enhance={() => async ({ update }) => { await update(); await invalidateAll() }}
>
	<input bind:this={renameIdInput} name="id" />
	<input bind:this={renameNameInput} name="name" />
	<input bind:this={renameSecIdInput} name="security_id" />
	<input bind:this={renameSegInput} name="exchange_segment" />
	<input bind:this={renameIntervalInput} name="interval" />
	<input bind:this={renameIndicatorsInput} name="indicators" />
</form>

<div class="layout">
	<div class="sidebar">
		<div class="sidebar-header">
			<span class="label">Charts</span>
			<button class="icon-btn" onclick={newChart} title="New chart">+</button>
		</div>
		<ul class="chart-list">
			{#each data.charts as c}
				<li class="chart-item" class:active={c.id === chartId}>
					{#if editingChartId === c.id}
						<input
							class="chart-rename"
							bind:value={editingName}
							onblur={() => commitRename(c)}
							onkeydown={(e) => { if (e.key === 'Enter') commitRename(c); if (e.key === 'Escape') editingChartId = null }}
							use:focus
						/>
					{:else}
						<div class="chart-row">
							<button onclick={() => selectChart(c)} class="chart-btn">{c.name}</button>
							<div class="menu-wrap">
								<button class="dots-btn" onclick={(e) => toggleMenu(c.id, e)}>···</button>
								{#if menuOpenId === c.id}
									<div class="menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
										<button onclick={() => { startRename(c); closeMenu() }}>Rename</button>
										<form method="POST" action="?/delete" use:enhance={() => async ({ update }) => { await update(); await invalidateAll() }}>
											<input type="hidden" name="id" value={c.id} />
											<button type="submit" class="danger">Delete</button>
										</form>
									</div>
								{/if}
							</div>
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	</div>

	<div class="main">
		<div class="toolbar">
			<InstrumentSearch onselect={selectInstrument} inputId="chart-instrument" />

			<select bind:value={interval} class="interval-select">
				{#each INTERVALS as iv}
					<option value={iv}>{iv}</option>
				{/each}
			</select>

			{#if !isMobile}
				<input type="date" class="date-input" bind:value={fromDate} min={IS_INTRADAY ? new Date(Date.now() - 5*365*24*60*60*1000).toISOString().slice(0,10) : undefined} max={toDate} onchange={() => instrument && fetchCandles()} disabled={live} />
				<span class="date-sep">→</span>
				<input type="date" class="date-input" bind:value={toDate} min={fromDate} onchange={() => instrument && fetchCandles()} disabled={live} />

				{#if IS_INTRADAY && instrument}
					<button
						class="live-btn"
						class:live-on={live}
						onclick={() => { live = !live }}
						title={live ? 'Stop live feed' : 'Start live feed'}
					>
						<span class="live-dot"></span>Live
					</button>
				{/if}
			{/if}

			<span class="spacer"></span>

			{#if isMobile}
				<button class="view-toggle" onclick={() => mobileView = mobileView === 'candles' ? 'volume' : 'candles'} title="Toggle view">
					{mobileView === 'candles' ? '≡' : '🕯'}
				</button>
				<button class="settings-btn" class:active={settingsOpen} onclick={() => settingsOpen = !settingsOpen} title="Settings">⚙</button>
			{:else}
				<input class="chart-name" bind:value={chartName} placeholder="Chart name" />
				<form method="POST" action="?/save" use:enhance={({ formData }) => {
					formData.set('indicators', JSON.stringify(indicators))
					if (chartId) formData.set('id', chartId)
					return async ({ result, update }) => {
						if (result.type === 'success' && result.data?.id) chartId = result.data.id as string
						await update()
						await invalidateAll()
					}
				}}>
					<input type="hidden" name="name" value={chartName} />
					<input type="hidden" name="security_id" value={instrument?.security_id ?? ''} />
					<input type="hidden" name="exchange_segment" value={instrument?.exchange_segment ?? ''} />
					<input type="hidden" name="interval" value={interval} />
					<button type="submit" class="btn-primary" disabled={!instrument}>Save chart</button>
				</form>
			{/if}
		</div>

		{#if isMobile && settingsOpen}
			<div class="mobile-settings">
				<input type="date" class="date-input" bind:value={fromDate} min={IS_INTRADAY ? new Date(Date.now() - 5*365*24*60*60*1000).toISOString().slice(0,10) : undefined} max={toDate} onchange={() => instrument && fetchCandles()} disabled={live} />
				<span class="date-sep">→</span>
				<input type="date" class="date-input" bind:value={toDate} min={fromDate} onchange={() => instrument && fetchCandles()} disabled={live} />
				{#if IS_INTRADAY && instrument}
					<button class="live-btn" class:live-on={live} onclick={() => { live = !live }}>
						<span class="live-dot"></span>Live
					</button>
				{/if}
			</div>
		{/if}

		{#if error}
			<p class="error">{error}</p>
		{/if}

		<div class="chart-wrap">
			<div bind:this={chartContainer} class="chart" class:loading></div>
			{#if loading}
				<div class="overlay">Loading…</div>
			{/if}
		</div>

		{#if candles.length > 0}
			<div class="scrollbar-wrap">
				<input
					type="range"
					class="scrollbar"
					min="0"
					max={scrollMax}
					bind:value={scrollPos}
					oninput={onScrollbarInput}
					style="--thumb-pct: {scrollThumb}%"
				/>
			</div>
		{/if}
	</div>

	{#if !isMobile}
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

		{#if instrument}
			<div class="panel-actions">
				<button onclick={designStrategy} class="btn-primary">Design strategy</button>
			</div>
		{/if}
	</div>
	{/if}
</div>

<style>
	.layout {
		display: flex;
		height: 100%;
		overflow: hidden;
	}

	.sidebar {
		background: var(--bg-surface);
		border-right: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		overflow: visible;
		position: relative;
		width: 160px;
		z-index: 10;
	}

	@media (max-width: 768px) {
		.sidebar { display: none; }
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

	.chart-row {
		align-items: center;
		display: flex;
		width: 100%;
	}

	.chart-row .chart-btn { flex: 1; }

	.menu-wrap { position: relative; }

	.dots-btn {
		background: none;
		border: none;
		border-radius: 4px;
		color: var(--text-faint);
		cursor: pointer;
		font-size: 0.9rem;
		letter-spacing: 1px;
		opacity: 0;
		padding: 4px 6px;
	}

	.chart-item:hover .dots-btn,
	.chart-item.active .dots-btn { opacity: 1; }
	.dots-btn:hover { background: var(--bg); color: var(--text); }

	.menu {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		bottom: auto;
		box-shadow: 0 4px 12px rgba(0,0,0,0.2);
		display: flex;
		flex-direction: column;
		left: 0;
		min-width: 100px;
		overflow: hidden;
		position: absolute;
		top: calc(100% + 4px);
		z-index: 100;
	}

	.menu button, .menu form { width: 100%; }

	.menu button {
		background: none;
		border: none;
		color: var(--text);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		padding: 8px 12px;
		text-align: left;
		width: 100%;
	}

	.menu button:hover { background: var(--bg); }
	.menu button.danger { color: var(--red); }
	.menu button.danger:hover { background: var(--bg); }

	.chart-rename {
		background: var(--bg-surface);
		border: 1px solid var(--accent);
		border-radius: 4px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		outline: none;
		padding: 5px 8px;
		width: 100%;
	}

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

	.chart-name {
		background: transparent;
		border: 1px solid transparent;
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		outline: none;
		padding: 5px 8px;
		width: 120px;
	}

	.chart-name:hover { border-color: var(--border); }
	.chart-name:focus { border-color: var(--accent); }

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

	.date-input {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.75rem;
		outline: none;
		padding: 5px 6px;
		width: 160px;
	}

	.date-input:focus { border-color: var(--accent); }

	.date-sep {
		color: var(--text-faint);
		font-size: 0.75rem;
	}

	.scrollbar-wrap {
		background: var(--bg-surface);
		border-top: 1px solid var(--border);
		padding: 4px 8px;
	}

	.scrollbar {
		-webkit-appearance: none;
		appearance: none;
		background: var(--border);
		border-radius: 2px;
		cursor: pointer;
		height: 6px;
		outline: none;
		width: 100%;
	}

	.scrollbar::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		background: var(--text-muted);
		border-radius: 2px;
		cursor: grab;
		height: 6px;
		width: var(--thumb-pct, 20%);
	}

	.scrollbar::-webkit-slider-thumb:active { cursor: grabbing; }

	.chart-wrap {
		flex: 1;
		overflow: visible;
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
		justify-content: flex-start;
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

	.panel-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: auto;
		padding-top: 12px;
	}

	.panel-actions .btn-primary { width: 100%; }

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

	.live-btn {
		align-items: center;
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		display: flex;
		font-family: 'Inter', sans-serif;
		font-size: 0.75rem;
		gap: 5px;
		padding: 5px 8px;
		white-space: nowrap;
	}

	.live-btn:hover { border-color: var(--text-muted); color: var(--text); }

	.live-btn.live-on {
		border-color: #22c55e;
		color: #22c55e;
	}

	.live-dot {
		background: var(--text-faint);
		border-radius: 50%;
		display: inline-block;
		height: 6px;
		width: 6px;
	}

	.live-btn.live-on .live-dot {
		animation: pulse 1.2s ease-in-out infinite;
		background: #22c55e;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
	}

	.view-toggle {
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		line-height: 1;
	}

	.settings-btn {
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		line-height: 1;
	}

	.settings-btn.active {
		border-color: var(--accent);
		color: var(--accent);
	}

	.mobile-settings {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 12px;
		background: var(--bg-surface);
		border-bottom: 1px solid var(--border);
		flex-wrap: wrap;
	}

	@media (max-width: 768px) {
		.toolbar {
			flex-wrap: nowrap;
			gap: 6px;
			padding: 8px 10px;
		}

		.chart-list-sidebar {
			display: none;
		}
	}
</style>
