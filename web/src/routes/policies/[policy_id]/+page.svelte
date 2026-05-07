<script lang="ts">
	import { enhance } from '$app/forms'

	let { data } = $props()
	const policy = $derived(data.policy)
</script>

<div class="header">
	<div>
		<a href="/policies" class="back">← Policies</a>
		<h1>{policy.strategy_name}</h1>
		<p class="meta">
			{policy.instruments?.filter((i: any) => i.trading_symbol).map((i: any) => i.trading_symbol).join(', ') || '—'}
			· {policy.mode}
		</p>
	</div>
	<div class="actions">
		<span class="badge {policy.status}">{policy.status}</span>
		<form method="POST" action="?/toggle" use:enhance>
			<button type="submit" class="btn-toggle {policy.status === 'active' ? 'pause' : 'activate'}">
				{policy.status === 'active' ? 'Pause' : 'Activate'}
			</button>
		</form>
	</div>
</div>

<section>
	<h2>Details</h2>
	<div class="details">
		<div class="detail-row">
			<span class="detail-label">Strategy</span>
			<a href="/strategies/{policy.strategy_id}" class="detail-value link">{policy.strategy_name}</a>
		</div>
		<div class="detail-row">
			<span class="detail-label">Mode</span>
			<span class="detail-value">{policy.mode}</span>
		</div>
		<div class="detail-row">
			<span class="detail-label">Instruments</span>
			<span class="detail-value">
				{policy.instruments?.filter((i: any) => i.trading_symbol).map((i: any) => `${i.trading_symbol} (${i.exchange_segment})`).join(', ') || '—'}
			</span>
		</div>
		<div class="detail-row">
			<span class="detail-label">Created</span>
			<span class="detail-value">{new Date(policy.created_at).toLocaleDateString('en-IN')}</span>
		</div>
	</div>
</section>

<style>
	.header {
		align-items: flex-start;
		display: flex;
		justify-content: space-between;
		margin-bottom: 32px;
	}

	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }

	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 4px; }

	.meta { color: var(--text-muted); font-size: 0.8rem; margin: 0; }

	.actions { align-items: center; display: flex; gap: 12px; }

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

	.btn-toggle {
		border-radius: 6px;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 16px;
	}

	.btn-toggle.pause {
		background: none;
		border: 1px solid var(--border);
		color: var(--text-muted);
	}

	.btn-toggle.pause:hover { color: var(--text); }

	.btn-toggle.activate {
		background: var(--accent);
		border: none;
		color: #000;
	}

	.btn-toggle.activate:hover { background: var(--accent-hover); }

	section { margin-bottom: 32px; }

	h2 {
		color: var(--text-muted);
		font-size: 0.8rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		margin: 0 0 16px;
		text-transform: uppercase;
	}

	.details {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		max-width: 480px;
		overflow: hidden;
	}

	.detail-row {
		align-items: baseline;
		border-bottom: 1px solid var(--border);
		display: flex;
		gap: 16px;
		padding: 12px 16px;
	}

	.detail-row:last-child { border-bottom: none; }

	.detail-label {
		color: var(--text-muted);
		flex-shrink: 0;
		font-size: 0.8rem;
		width: 100px;
	}

	.detail-value { color: var(--text); font-size: 0.875rem; }

	.link { color: var(--accent); text-decoration: none; }
	.link:hover { text-decoration: underline; }
</style>
