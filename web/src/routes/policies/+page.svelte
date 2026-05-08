<script lang="ts">
	import { enhance } from '$app/forms'
	import { goto } from '$app/navigation'

	let { data } = $props()

	function filterByStrategy(e: Event) {
		const id = (e.target as HTMLSelectElement).value
		goto(id ? `/policies?strategy_id=${id}` : '/policies')
	}
</script>

<div class="header">
	<h1>Policies</h1>
	<div class="actions">
		<select onchange={filterByStrategy}>
			<option value="">All strategies</option>
			{#each data.strategies as s}
				<option value={s.id} selected={s.id === data.selectedStrategyId}>{s.name}</option>
			{/each}
		</select>
		<a href="/policies/new" class="btn-primary">New policy</a>
	</div>
</div>

{#if data.policies.length === 0}
	<p class="empty">No policies yet.</p>
{:else}
	<ul class="list">
		{#each data.policies as policy}
			<li>
				<a href="/policies/{policy.id}" class="row">
					<div class="row-left">
						<span class="strategy">{policy.strategy_name}</span>
						<span class="symbols">{policy.symbols?.filter(Boolean).join(', ') || '—'}</span>
						<span class="meta">{policy.mode} · {policy.interval}</span>
					</div>
					<span class="badge {policy.status}">{policy.status}</span>
				</a>
				<form method="POST" action="?/toggle" use:enhance>
					<input type="hidden" name="id" value={policy.id} />
					<input type="hidden" name="status" value={policy.status} />
					<button type="submit" class="icon-btn" title={policy.status === 'active' ? 'Pause' : 'Activate'}>
						{policy.status === 'active' ? '⏸' : '▶'}
					</button>
				</form>
				<form method="POST" action="?/delete" use:enhance onsubmit={(e) => { if (!confirm('Delete this policy?')) e.preventDefault() }}>
					<input type="hidden" name="id" value={policy.id} />
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

	li { align-items: stretch; display: flex; gap: 6px; }
	li .row { flex: 1; }

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

	.badge {
		border-radius: 4px;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		padding: 2px 8px;
		text-transform: uppercase;
	}

	.badge.active { background: var(--green); color: #000; }
	.badge.paused { background: var(--bg); color: var(--text-muted); }

	.icon-btn, .del-btn {
		background: none;
		border: 1px solid var(--border);
		border-radius: 8px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.75rem;
		padding: 0 10px;
	}

	.icon-btn:hover { border-color: var(--accent); color: var(--accent); }
	.del-btn:hover { border-color: var(--red-muted); color: var(--red-muted); }

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
</style>
