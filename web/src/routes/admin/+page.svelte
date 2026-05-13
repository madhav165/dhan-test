<script lang="ts">
	let { data } = $props()

	const { summary: initSummary, failed: initFailed } = data.stats

	let summary = $state({ ...initSummary })
	let failed = $state({ ...initFailed })
	let isDone = $state(false)
	let connected = $state(false)

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

	let ws: WebSocket | null = null

	function connect() {
		if (!data.goWsUrl || !data.wsToken) return
		ws = new WebSocket(`${data.goWsUrl}/admin/ohlcv/ws?token=${data.wsToken}`)
		connected = true

		ws.onmessage = ({ data: raw }) => {
			const msg = JSON.parse(raw)
			if (msg.done === true) {
				isDone = true
				return
			}
			if (msg.pending !== undefined) {
				summary = { ...msg }
			}
		}
		ws.onclose = () => {
			connected = false
			if (!isDone) {
				setTimeout(connect, 3000)
			}
		}
		ws.onerror = () => {
			connected = false
		}
	}

	connect()

	$effect(() => {
		return () => {
			ws?.close()
		}
	})

	const total = summary.total || 0
	const progress = total > 0 ? Math.round((summary.done || 0) / total * 100) : 0
</script>

<div class="header">
	<h1>Admin</h1>
	{#if connected}
		<span class="badge live">live</span>
	{:else if isDone}
		<span class="badge idle">idle</span>
	{:else}
		<span class="badge connecting">connecting</span>
	{/if}
</div>

{#if total > 0}
	<section>
		<h2>Progress</h2>
		<div class="progress-bar">
			<div class="progress-fill" style="width: {progress}%"></div>
		</div>
		<span class="progress-label">{summary.done || 0} / {total.toLocaleString()} ({progress}%)</span>
	</section>
{/if}

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
		gap: 12px;
		margin-bottom: 24px;
	}

	h1 {
		font-size: 20px;
		font-weight: 600;
		margin: 0;
	}

	.badge {
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		padding: 2px 8px;
		border-radius: 4px;
	}

	.badge.live {
		color: var(--green);
		background: var(--green)11;
	}

	.badge.idle {
		color: var(--text-muted);
		background: var(--text-muted)11;
	}

	.badge.connecting {
		color: var(--accent);
		background: var(--accent)11;
		animation: pulse 1.5s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.4; }
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

	.progress-bar {
		height: 6px;
		background: var(--border);
		border-radius: 3px;
		overflow: hidden;
		margin-bottom: 8px;
	}

	.progress-fill {
		height: 100%;
		background: var(--accent);
		border-radius: 3px;
		transition: width 0.3s ease;
	}

	.progress-label {
		font-size: 12px;
		color: var(--text-muted);
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

	
</style>
