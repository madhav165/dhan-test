<script lang="ts">
	let { data } = $props()

	const { summary, failed } = data.stats

	const statusOrder = ['pending', 'running', 'done', 'failed']
	const failedOrder = ['rate_limited', 'no_data', 'token_error', 'other']

	const statusLabel: Record<string, string> = {
		pending: 'Pending',
		running: 'Running',
		done: 'Done',
		failed: 'Failed'
	}

	const failedLabel: Record<string, string> = {
		rate_limited: 'Rate limited (429)',
		no_data: 'No data (400)',
		token_error: 'Token error',
		other: 'Other'
	}
</script>

<div class="header">
	<h1>Admin</h1>
</div>

<nav class="admin-nav">
	<a href="/admin/ohlcv">OHLCV stocks</a>
</nav>

<section>
	<h2>OHLCV Jobs</h2>
	<table>
		<thead>
			<tr>
				<th>Status</th>
				<th>Count</th>
			</tr>
		</thead>
		<tbody>
			{#each statusOrder as s}
				{#if summary[s] != null}
					<tr class={s}>
						<td>{statusLabel[s]}</td>
						<td>{summary[s].toLocaleString()}</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
</section>

{#if Object.keys(failed).length > 0}
	<section>
		<h2>Failed breakdown</h2>
		<table>
			<thead>
				<tr>
					<th>Type</th>
					<th>Count</th>
				</tr>
			</thead>
			<tbody>
				{#each failedOrder as t}
					{#if failed[t] != null}
						<tr class={t === 'rate_limited' ? 'rate-limited' : t === 'no_data' ? 'no-data' : ''}>
							<td>{failedLabel[t]}</td>
							<td>{failed[t].toLocaleString()}</td>
						</tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</section>
{/if}

<style>
	.header {
		display: flex;
		align-items: center;
		margin-bottom: 24px;
	}

	h1 {
		font-size: 20px;
		font-weight: 600;
		margin: 0;
	}

	h2 {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-subtle);
		letter-spacing: 0.05em;
		text-transform: uppercase;
		margin: 0 0 12px;
	}

	section {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		margin-bottom: 24px;
		padding: 20px;
		max-width: 400px;
	}

	table {
		border-collapse: collapse;
		width: 100%;
	}

	th {
		color: var(--text-faint);
		font-size: 12px;
		font-weight: 500;
		padding: 0 0 8px;
		text-align: left;
	}

	th:last-child, td:last-child {
		text-align: right;
	}

	td {
		border-top: 1px solid var(--border);
		font-size: 14px;
		padding: 10px 0;
	}

	tr.done td { color: var(--green); }
	tr.running td { color: var(--accent); }
	tr.failed td, tr.rate-limited td { color: var(--red); }
	tr.pending td { color: var(--text-subtle); }
	tr.no-data td { color: var(--text-muted); }

	.admin-nav {
		display: flex;
		gap: 12px;
		margin-bottom: 24px;
	}

	.admin-nav a {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-subtle);
		font-size: 13px;
		padding: 6px 14px;
		text-decoration: none;
		transition: border-color 0.15s, color 0.15s;
	}

	.admin-nav a:hover {
		border-color: var(--accent);
		color: var(--text);
	}
</style>
