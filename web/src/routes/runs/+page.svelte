<script lang="ts">
	import { goto } from '$app/navigation'

	let { data } = $props()

	function filterByStrategy(e: Event) {
		const id = (e.target as HTMLSelectElement).value
		goto(id ? `/runs?strategy_id=${id}` : '/runs')
	}
</script>

<div class="header">
	<h1>Runs</h1>
	<div class="actions">
		<select onchange={filterByStrategy}>
			<option value="">All strategies</option>
			{#each data.strategies as s}
				<option value={s.id} selected={s.id === data.selectedStrategyId}>{s.name}</option>
			{/each}
		</select>
		<a href="/runs/new" class="btn-primary">New run</a>
	</div>
</div>

{#if data.runs.length === 0}
	<p class="empty">No runs yet.</p>
{:else}
	<ul class="list">
		{#each data.runs as run}
			<li>
				<a href="/runs/{run.id}" class="row">
					<div class="row-left">
						<span class="strategy">{run.strategy_name}</span>
						<span class="symbols">{run.symbols?.filter(Boolean).join(', ') || '—'}</span>
						<span class="meta">{run.interval} · {new Date(run.from_date).toISOString().slice(0, 10)} → {new Date(run.to_date).toISOString().slice(0, 10)}</span>
					</div>
					<div class="right">
						<div class="metrics">
							<span>Trades: {run.num_trades ?? '—'}</span>
							<span>PnL: {run.total_pnl != null ? Number(run.total_pnl).toFixed(2) : '—'}</span>
							<span>Win: {run.win_rate != null ? (run.win_rate * 100).toFixed(1) + '%' : '—'}</span>
							<span>DD: {run.max_drawdown != null ? Number(run.max_drawdown).toFixed(2) : '—'}</span>
						</div>
						<span class="badge {run.job_status}">{run.job_status ?? 'unknown'}</span>
					</div>
				</a>
				<form method="POST" action="?/delete" onsubmit={(e) => { if (!confirm('Delete this run?')) e.preventDefault() }}>
					<input type="hidden" name="id" value={run.id} />
					<button type="submit" class="del-btn">✕</button>
				</form>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.header {
		align-items: center;
		display: flex;
		justify-content: space-between;
		margin-bottom: 24px;
	}

	h1 { font-size: 1.25rem; font-weight: 600; margin: 0; }

	.actions { align-items: center; display: flex; gap: 12px; }

	select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.8rem;
		outline: none;
		padding: 6px 10px;
	}

	.empty { color: var(--text-muted); font-size: 0.875rem; }

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

	.row-left { display: flex; flex-direction: column; gap: 3px; }

	.strategy { color: var(--text); font-size: 0.875rem; font-weight: 600; }
	.symbols { color: var(--text); font-size: 0.8rem; }
	.meta { color: var(--text-muted); font-size: 0.75rem; }

	.right { align-items: flex-end; display: flex; flex-direction: column; gap: 8px; }

	.metrics {
		color: var(--text-muted);
		display: flex;
		font-size: 0.8rem;
		gap: 16px;
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
	.badge.pending, .badge.ready { background: var(--accent); color: #000; }
	.badge.running { background: var(--accent); color: #000; }
	.badge.failed { background: var(--red-bg); color: var(--red-muted); }

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

	li { display: flex; align-items: stretch; gap: 6px; }
	li .row { flex: 1; }

	.del-btn {
		background: none;
		border: 1px solid var(--border);
		border-radius: 8px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.75rem;
		padding: 0 10px;
	}

	.del-btn:hover { border-color: var(--red-muted); color: var(--red-muted); }
</style>
