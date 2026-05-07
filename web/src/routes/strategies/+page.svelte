<script lang="ts">
	let { data } = $props()
</script>

<div class="header">
	<h1>Strategies</h1>
	<a href="/strategies/new" class="btn-primary">New strategy</a>
</div>

{#if data.strategies.length === 0}
	<p class="empty">No strategies yet. Create one to get started.</p>
{:else}
	<ul class="list">
		{#each data.strategies as s}
			<li>
				<a href="/strategies/{s.id}" class="strategy-row">
					<div class="strategy-info">
						<span class="name">{s.name}</span>
						<span class="meta">Created {new Date(s.created_at).toLocaleDateString('en-IN')}</span>
					</div>
					<span class="badge {s.wasm_key ? 'ready' : 'draft'}">{s.wasm_key ? 'WASM ready' : 'No WASM'}</span>
				</a>
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

	h1 {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0;
	}

	.btn-primary {
		background: var(--accent);
		border-radius: 6px;
		color: #000;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 16px;
		text-decoration: none;
		transition: background 0.15s;
	}

	.btn-primary:hover {
		background: var(--accent-hover);
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

	.strategy-row {
		align-items: center;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		display: flex;
		justify-content: space-between;
		padding: 16px;
		text-decoration: none;
		transition: border-color 0.15s;
	}

	.strategy-row:hover {
		border-color: var(--accent);
	}

	.strategy-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.name {
		color: var(--text);
		font-size: 0.9rem;
		font-weight: 500;
	}

	.meta {
		color: var(--text-muted);
		font-size: 0.75rem;
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
	.badge.ready { background: var(--green); color: #000; }
</style>
