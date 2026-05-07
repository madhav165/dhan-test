<script lang="ts">
	let { data } = $props()
	const strategy = $derived(data.strategy)
	const instruments = $derived(data.instruments)
	const runs = $derived(data.runs)
</script>

<div class="header">
	<div>
		<a href="/strategies" class="back">← Strategies</a>
		<h1>{strategy.name}</h1>
		<p class="meta">{strategy.interval} · {strategy.from_date} → {strategy.to_date}</p>
	</div>
	<div class="actions">
		<span class="badge {strategy.status}">{strategy.status}</span>
		<a href="/strategies/{strategy.id}/run" class="btn-primary">New run</a>
	</div>
</div>

<section>
	<h2>Instruments</h2>
	{#if instruments.length === 0}
		<p class="empty">No instruments selected.</p>
	{:else}
		<ul class="instrument-list">
			{#each instruments as inst}
				<li>
					<span class="symbol">{inst.trading_symbol}</span>
					{#if inst.custom_symbol}
						<span class="iname">{inst.custom_symbol}</span>
					{/if}
					<span class="tag">{inst.exchange_segment}</span>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<section>
	<h2>Runs</h2>
	{#if runs.length === 0}
		<p class="empty">No runs yet. Click "New run" to backtest this strategy.</p>
	{:else}
		<ul class="list">
			{#each runs as run}
				<li>
					<a href="/strategies/{strategy.id}/runs/{run.id}" class="run-row">
						<span class="run-date">{new Date(run.run_at).toLocaleString('en-IN')}</span>
						<div class="run-metrics">
							<span>Trades: {run.num_trades ?? '—'}</span>
							<span>PnL: {run.total_pnl != null ? run.total_pnl : '—'}</span>
							<span>Win rate: {run.win_rate != null ? (run.win_rate * 100).toFixed(1) + '%' : '—'}</span>
							<span>Drawdown: {run.max_drawdown != null ? run.max_drawdown : '—'}</span>
						</div>
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

	section { margin-bottom: 32px; }

	h2 {
		font-size: 0.9rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		margin: 0 0 12px;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	.empty {
		color: var(--text-muted);
		font-size: 0.875rem;
	}

	.instrument-list {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.instrument-list li {
		align-items: center;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		display: flex;
		gap: 8px;
		padding: 6px 12px;
	}

	.symbol { font-size: 0.875rem; font-weight: 600; }
	.iname { color: var(--text-muted); font-size: 0.8rem; }
	.tag {
		background: var(--bg);
		border-radius: 4px;
		color: var(--text-muted);
		font-size: 0.7rem;
		padding: 2px 6px;
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.run-row {
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

	.run-row:hover { border-color: var(--accent); }

	.run-date {
		color: var(--text-muted);
		font-size: 0.8rem;
	}

	.run-metrics {
		color: var(--text);
		display: flex;
		font-size: 0.8rem;
		gap: 20px;
	}

	.badge {
		border-radius: 4px;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		padding: 2px 8px;
		text-transform: uppercase;
	}

	.badge.draft { background: var(--bg); color: var(--text-muted); }
	.badge.backtesting { background: var(--accent); color: #000; }
	.badge.active { background: var(--green); color: #000; }
</style>
