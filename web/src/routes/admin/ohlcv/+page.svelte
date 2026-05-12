<script lang="ts">
	import { goto } from '$app/navigation'
	import { page } from '$app/stores'

	let { data } = $props()

	let q        = $state(data.q)
	let industry = $state(data.industry)

	let debounce: ReturnType<typeof setTimeout>

	function navigate(newPage = 1) {
		const params = new URLSearchParams()
		if (q)        params.set('q', q)
		if (industry) params.set('industry', industry)
		if (newPage > 1) params.set('page', String(newPage))
		goto(`/admin/ohlcv?${params}`, { keepFocus: true })
	}

	function onSearch() {
		clearTimeout(debounce)
		debounce = setTimeout(() => navigate(1), 300)
	}

	function onIndustry(e: Event) {
		industry = (e.target as HTMLSelectElement).value
		navigate(1)
	}

	const totalPages = $derived(Math.ceil(data.total / data.page_size))
</script>

<div class="header">
	<h1>OHLCV Stocks</h1>
	<span class="total">{data.total} stocks</span>
</div>

<div class="filters">
	<input
		type="search"
		placeholder="Search symbol or name…"
		bind:value={q}
		oninput={onSearch}
	/>
	<select onchange={onIndustry} value={industry}>
		<option value="">All industries</option>
		{#each data.industries as ind}
			<option value={ind} selected={ind === industry}>{ind}</option>
		{/each}
	</select>
</div>

<div class="table-wrap">
	<table>
		<thead>
			<tr>
				<th>Symbol</th>
				<th>Company</th>
				<th>Industry</th>
				<th>Start date</th>
				<th>End date</th>
				<th class="num">Chunks</th>
				<th class="num">Done</th>
				<th class="num">Pending</th>
				<th class="num">Failed</th>
			</tr>
		</thead>
		<tbody>
			{#each data.stocks as s}
				<tr class:has-failed={s.failed > 0}>
					<td class="symbol">{s.symbol}</td>
					<td class="name">{s.company_name}</td>
					<td class="industry">{s.industry}</td>
					<td>{s.start_date}</td>
					<td>{s.end_date}</td>
					<td class="num">{s.chunks}</td>
					<td class="num done">{s.done}</td>
					<td class="num">{s.pending}</td>
					<td class="num" class:red={s.failed > 0}>{s.failed}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

{#if totalPages > 1}
	<div class="pagination">
		<button disabled={data.page <= 1} onclick={() => navigate(data.page - 1)}>
			Previous
		</button>
		<span>Page {data.page} of {totalPages}</span>
		<button disabled={data.page >= totalPages} onclick={() => navigate(data.page + 1)}>
			Next
		</button>
	</div>
{/if}

<style>
	.header {
		align-items: baseline;
		display: flex;
		gap: 12px;
		margin-bottom: 20px;
	}

	h1 {
		font-size: 20px;
		font-weight: 600;
		margin: 0;
	}

	.total {
		color: var(--text-muted);
		font-size: 13px;
	}

	.filters {
		display: flex;
		gap: 12px;
		margin-bottom: 16px;
	}

	input[type="search"] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: inherit;
		font-size: 13px;
		outline: none;
		padding: 7px 10px;
		width: 220px;
	}

	input[type="search"]:focus {
		border-color: var(--accent);
	}

	select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: inherit;
		font-size: 13px;
		outline: none;
		padding: 7px 10px;
		min-width: 160px;
	}

	select:focus {
		border-color: var(--accent);
	}

	.table-wrap {
		overflow-x: auto;
	}

	table {
		border-collapse: collapse;
		font-size: 13px;
		width: 100%;
	}

	th {
		border-bottom: 1px solid var(--border);
		color: var(--text-faint);
		font-size: 11px;
		font-weight: 500;
		padding: 0 12px 8px 0;
		text-align: left;
		white-space: nowrap;
	}

	th.num, td.num {
		text-align: right;
		padding-right: 0;
		padding-left: 16px;
	}

	td {
		border-bottom: 1px solid var(--border);
		color: var(--text);
		padding: 9px 12px 9px 0;
		white-space: nowrap;
	}

	.symbol {
		font-weight: 500;
	}

	.name {
		color: var(--text-subtle);
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.industry {
		color: var(--text-muted);
		font-size: 12px;
	}

	td.done { color: var(--green); }
	td.red  { color: var(--red); }

	.pagination {
		align-items: center;
		display: flex;
		gap: 16px;
		justify-content: center;
		margin-top: 24px;
	}

	.pagination span {
		color: var(--text-muted);
		font-size: 13px;
	}

	.pagination button {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		cursor: pointer;
		font-family: inherit;
		font-size: 13px;
		padding: 6px 14px;
		transition: background 0.15s;
	}

	.pagination button:hover:not(:disabled) {
		background: var(--bg);
		border-color: var(--accent);
	}

	.pagination button:disabled {
		color: var(--text-faint);
		cursor: default;
	}
</style>
