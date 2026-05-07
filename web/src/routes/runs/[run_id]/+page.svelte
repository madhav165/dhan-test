<script lang="ts">
	import { invalidateAll } from '$app/navigation'
	import { onDestroy } from 'svelte'

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

	onDestroy(() => clearInterval(timer))

	function fmt(v: number | null, decimals = 2) {
		return v != null ? Number(v).toFixed(decimals) : '—'
	}
</script>

<div class="header">
	<div>
		<a href="/runs" class="back">← Runs</a>
		<h1>{run.strategy_name}</h1>
		<p class="meta">
			{run.instruments?.filter((i: any) => i.trading_symbol).map((i: any) => i.trading_symbol).join(', ') || '—'}
			· {run.interval} · {run.from_date} → {run.to_date}
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
</style>
