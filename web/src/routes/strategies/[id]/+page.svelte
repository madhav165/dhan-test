<script lang="ts">
	import { enhance } from '$app/forms'
	import { invalidateAll } from '$app/navigation'
	import { onDestroy } from 'svelte'

	let { data } = $props()
	const strategy = $derived(data.strategy)
	const runs = $derived(data.runs)
	const policies = $derived(data.policies)

	const building = $derived(
		strategy.build_status === 'pending' || strategy.build_status === 'building'
	)

	let timer: ReturnType<typeof setInterval>
	$effect(() => {
		if (building) {
			timer = setInterval(() => invalidateAll(), 3000)
		} else {
			clearInterval(timer)
		}
	})

	onDestroy(() => clearInterval(timer))
</script>

<div class="header">
	<div>
		<a href="/strategies" class="back">← Strategies</a>
		<h1>{strategy.name}</h1>
		<p class="meta">Created {new Date(strategy.created_at).toLocaleDateString('en-IN')}</p>
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
</style>
