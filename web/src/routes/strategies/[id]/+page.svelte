<script lang="ts">
	import { enhance } from '$app/forms'
	import { invalidateAll } from '$app/navigation'
	import { onDestroy } from 'svelte'
	import { createChart, LineSeries, ColorType, type IChartApi, type ISeriesApi, type LineData } from 'lightweight-charts'

	let { data } = $props()
	const strategy = $derived(data.strategy)
	const runs = $derived(data.runs)
	const policies = $derived(data.policies)
	const rlMetrics = $derived(data.rl_metrics ?? [])

	const building = $derived(
		strategy.build_status === 'pending' || strategy.build_status === 'building'
	)
	const rlPending = $derived(
		strategy.rl_job?.status === 'pending' || strategy.rl_job?.status === 'training'
	)

	let timer: ReturnType<typeof setInterval>
	$effect(() => {
		if (building || rlPending) {
			timer = setInterval(() => invalidateAll(), 3000)
		} else {
			clearInterval(timer)
		}
	})

	onDestroy(() => { clearInterval(timer); chart?.remove() })

	let chartContainer: HTMLDivElement = $state() as HTMLDivElement
	let chart: IChartApi
	let trainSeries: ISeriesApi<'Line'>
	let valSeries: ISeriesApi<'Line'>

	$effect(() => {
		if (!chartContainer || rlMetrics.length === 0) return
		if (chart) chart.remove()

		chart = createChart(chartContainer, {
			autoSize: true,
			layout: { textColor: '#ccc', background: { type: ColorType.Solid, color: 'transparent' } },
			grid: { vertLines: { color: '#333' }, horzLines: { color: '#333' } },
			leftPriceScale: { visible: true },
			rightPriceScale: { visible: false },
			timeScale: {
				tickMarkFormatter: (time: number) => String(time),
			},
		})

		trainSeries = chart.addSeries(LineSeries, { color: '#60a5fa', lineWidth: 2, title: 'Train reward' })
		valSeries = chart.addSeries(LineSeries, { color: '#f87171', lineWidth: 2, title: 'Val metric' })

		const trainData = rlMetrics.map((m: { episode: number; train_reward: number }) => ({
			time: m.episode as unknown as import('lightweight-charts').Time,
			value: m.train_reward,
		})) as LineData[]
		const valData = rlMetrics
			.filter((m: { val_metric: number | null }) => m.val_metric != null)
			.map((m: { episode: number; val_metric: number }) => ({
				time: m.episode as unknown as import('lightweight-charts').Time,
				value: m.val_metric,
			})) as LineData[]

		trainSeries.setData(trainData)
		valSeries.setData(valData)
		chart.timeScale().fitContent()
	})
</script>

<div class="header">
	<div>
		<a href="/strategies" class="back">← Strategies</a>
		<h1>{strategy.name}</h1>
		<p class="meta">Created {new Date(strategy.created_at).toLocaleDateString('en-IN')}{#if strategy.interval} · {strategy.interval}{/if}</p>
	</div>
	<div class="actions">
		{#if strategy.wasm_key}
			<span class="badge ready">Ready</span>
		{:else if strategy.build_status === 'pending' || strategy.build_status === 'building'}
			<span class="badge building">Building…</span>
		{:else if strategy.build_status === 'failed'}
			<span class="badge failed" title={strategy.build_error}>Build failed</span>
		{:else}
			<span class="badge draft">No WASM</span>
		{/if}
		<a href="/strategies/{strategy.id}/edit" class="btn-secondary">Edit</a>
		{#if strategy.wasm_key}
			<a href="/strategies/{strategy.id}/run" class="btn-primary">New run</a>
		{/if}
		<form method="POST" action="?/delete" use:enhance onsubmit={(e) => { if (!confirm('Delete this strategy and all its runs and policies?')) e.preventDefault() }}>
			<button type="submit" class="btn-delete">Delete</button>
		</form>
	</div>
</div>

{#if strategy.strategy_type === 'rl'}
	<section class="rl-section">
		<h2>RL Training</h2>
		{#if strategy.rl_summary?.split}
			{@const split = strategy.rl_summary.split}
			<p class="split-note">
				Train: {split.train_from} → {split.train_to} ({split.train_rows} rows)
				<span>Validation: {split.val_from} → {split.val_to} ({split.val_rows} rows)</span>
				<span>Test: {split.test_from} → {split.test_to} ({split.test_rows} rows)</span>
			</p>
		{:else if strategy.rl_config}
			<p class="split-note">
				Learning range: {strategy.rl_config.train_from} → {strategy.rl_config.train_to}
				<span>Split: 70% train / 15% validation / 15% test</span>
			</p>
		{/if}

		{#if strategy.rl_job?.status === 'pending' || strategy.rl_job?.status === 'training'}
			<p class="status-badge training">Training in progress…</p>
		{:else if strategy.rl_job?.status === 'failed'}
			<p class="status-badge failed">Training failed: {strategy.rl_job.error}</p>
		{:else if strategy.rl_summary}
			{@const summary = strategy.rl_summary}
			<div class="summary-grid">
				<div class="summary-item">
					<span class="summary-label">Episodes</span>
					<span class="summary-value">{summary.training_episodes}{summary.best_episode ? ` best ${summary.best_episode}` : ''}</span>
				</div>
				{#if summary.train_pnl != null}
				<div class="summary-item">
					<span class="summary-label">Train PnL</span>
					<span class="summary-value" class:pos={summary.train_pnl > 0} class:neg={summary.train_pnl < 0}>
						{summary.train_pnl.toFixed(2)}
					</span>
				</div>
				{/if}
				<div class="summary-item">
					<span class="summary-label">Train reward</span>
					<span class="summary-value">{summary.final_train_reward?.toFixed(4) ?? '—'}</span>
				</div>
				<div class="summary-item">
					<span class="summary-label">Val PnL</span>
					<span class="summary-value" class:pos={summary.val_pnl > 0} class:neg={summary.val_pnl < 0}>
						{summary.val_pnl != null ? summary.val_pnl.toFixed(2) : '—'}
					</span>
				</div>
				{#if summary.test_pnl != null}
				<div class="summary-item">
					<span class="summary-label">Test PnL</span>
					<span class="summary-value" class:pos={summary.test_pnl > 0} class:neg={summary.test_pnl < 0}>
						{summary.test_pnl.toFixed(2)}
					</span>
				</div>
				{/if}
			</div>

			{#if rlMetrics.length > 0}
				<h3>Training curves</h3>
				<div class="chart-wrap" bind:this={chartContainer}></div>
			{/if}

			<h3>Feature importance</h3>
			<div class="feature-list">
				{#each summary.feature_importance as f}
					<div class="feature-row">
						<span class="feature-name">{f.name}</span>
						<div class="feature-bar-wrap">
							<div class="feature-bar" style="width: {(f.importance * 100).toFixed(1)}%"></div>
						</div>
						<span class="feature-pct">{(f.importance * 100).toFixed(1)}%</span>
					</div>
				{/each}
			</div>

			<h3>Approximate rules</h3>
			<pre class="rules-pre">{summary.approximate_rules}</pre>
		{:else}
			<p class="status-badge pending">Waiting to start…</p>
		{/if}
	</section>
{/if}

<section>
	<div class="section-header">
		<h2>Runs</h2>
	</div>
	{#if runs.length === 0}
		<p class="empty">No runs yet. Click "New run" to backtest this strategy.</p>
	{:else}
		<ul class="list">
			{#each runs as run}
				<li>
					<a href="/runs/{run.id}" class="row">
						<div class="row-left">
							<span class="symbols">{run.symbols?.filter(Boolean).join(', ') || '—'}</span>
							<span class="meta">{run.interval} · {run.from_date} → {run.to_date}</span>
						</div>
						<div class="metrics">
							<span>Trades: {run.num_trades ?? '—'}</span>
							<span>PnL: {run.total_pnl ?? '—'}</span>
							<span>Win: {run.win_rate != null ? (run.win_rate * 100).toFixed(1) + '%' : '—'}</span>
							<span>DD: {run.max_drawdown ?? '—'}</span>
						</div>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<section>
	<div class="section-header">
		<h2>Policies</h2>
		<a href="/strategies/{strategy.id}/policies/new" class="btn-secondary">New policy</a>
	</div>
	{#if policies.length === 0}
		<p class="empty">No policies yet. Activate this strategy on instruments after a successful run.</p>
	{:else}
		<ul class="list">
			{#each policies as policy}
				<li>
					<a href="/policies/{policy.id}" class="row">
						<div class="row-left">
							<span class="symbols">{policy.symbols?.filter(Boolean).join(', ') || '—'}</span>
							<span class="meta">{policy.mode}</span>
						</div>
						<span class="badge {policy.status}">{policy.status}</span>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</section>

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

	.actions {
		align-items: center;
		display: flex;
		gap: 12px;
	}

	section { margin-bottom: 32px; }

	.section-header {
		align-items: center;
		display: flex;
		justify-content: space-between;
		margin-bottom: 12px;
	}

	h2 {
		color: var(--text-muted);
		font-size: 0.8rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		margin: 0;
		text-transform: uppercase;
	}

	.empty {
		color: var(--text-muted);
		font-size: 0.875rem;
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.row {
		align-items: center;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		display: flex;
		justify-content: space-between;
		padding: 14px 16px;
		text-decoration: none;
		transition: border-color 0.15s;
	}

	.row:hover { border-color: var(--accent); }

	.row-left {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.symbols {
		color: var(--text);
		font-size: 0.875rem;
		font-weight: 500;
	}

	.metrics {
		color: var(--text-muted);
		display: flex;
		font-size: 0.8rem;
		gap: 20px;
	}

	.btn-primary {
		background: var(--accent);
		border-radius: 6px;
		color: #000;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 16px;
		text-decoration: none;
	}

	.btn-primary:hover { background: var(--accent-hover); }

	.btn-delete {
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 16px;
	}

	.btn-delete:hover { border-color: var(--red-muted); color: var(--red-muted); }

	.btn-secondary {
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		font-size: 0.8rem;
		padding: 6px 12px;
		text-decoration: none;
	}

	.btn-secondary:hover { color: var(--text); }

	.badge {
		border-radius: 4px;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		padding: 2px 8px;
		text-transform: uppercase;
	}

	.badge.ready { background: var(--green); color: #000; }
	.badge.draft { background: var(--bg); color: var(--text-muted); }
	.badge.building { background: var(--accent); color: #000; }
	.badge.failed { background: var(--red-bg); color: var(--red-muted); cursor: help; }
	.badge.active { background: var(--green); color: #000; }
	.badge.paused { background: var(--bg); color: var(--text-muted); }

	.rl-section { margin-bottom: 32px; }
	.rl-section h2 { color: var(--text-muted); font-size: 0.8rem; font-weight: 600; letter-spacing: 0.06em; margin: 0 0 12px; text-transform: uppercase; }
	.rl-section h3 { color: var(--text-muted); font-size: 0.8rem; font-weight: 600; margin: 16px 0 8px; text-transform: uppercase; letter-spacing: 0.05em; }
	.split-note { color: var(--text-muted); display: flex; flex-wrap: wrap; gap: 8px 14px; font-size: 0.75rem; margin: -4px 0 14px; }

	.status-badge { border-radius: 4px; display: inline-block; font-size: 0.85rem; padding: 6px 12px; }
	.status-badge.training { background: #1e40af22; color: #60a5fa; }
	.status-badge.failed { background: var(--red-bg); color: var(--red-muted); }
	.status-badge.pending { background: var(--bg-surface); color: var(--text-muted); }

	.summary-grid { display: flex; gap: 16px; }
	.summary-item { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; padding: 12px 16px; }
	.summary-label { color: var(--text-muted); display: block; font-size: 0.75rem; margin-bottom: 4px; }
	.summary-value { font-size: 1.25rem; font-weight: 600; }
	.summary-value.pos { color: var(--green); }
	.summary-value.neg { color: var(--red-muted); }

	.feature-list { display: flex; flex-direction: column; gap: 6px; }
	.feature-row { align-items: center; display: flex; gap: 10px; }
	.feature-name { color: var(--text-muted); font-size: 0.8rem; min-width: 120px; }
	.feature-bar-wrap { background: var(--bg-surface); border-radius: 3px; flex: 1; height: 6px; }
	.feature-bar { background: var(--accent); border-radius: 3px; height: 100%; }
	.feature-pct { color: var(--text-muted); font-size: 0.75rem; min-width: 36px; text-align: right; }

	.rules-pre { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; color: var(--text); font-size: 0.8rem; line-height: 1.6; overflow-x: auto; padding: 12px; white-space: pre-wrap; }

	.chart-wrap { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; height: 240px; margin-bottom: 8px; }
</style>
