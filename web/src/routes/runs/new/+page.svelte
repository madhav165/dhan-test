<script lang="ts">
	import { enhance } from '$app/forms'
	import InstrumentSearch from '$lib/components/InstrumentSearch.svelte'

	let { data, form } = $props()

	type Instrument = {
		security_id: string
		exchange_segment: string
		trading_symbol: string
		custom_symbol: string
	}

	let selected = $state<Instrument[]>([])

	function add(inst: Instrument) {
		if (!selected.find((s) => s.security_id === inst.security_id && s.exchange_segment === inst.exchange_segment)) {
			selected = [...selected, inst]
		}
	}

	function remove(inst: Instrument) {
		selected = selected.filter(
			(s) => !(s.security_id === inst.security_id && s.exchange_segment === inst.exchange_segment)
		)
	}
</script>

<div class="header">
	<a href="/runs" class="back">← Runs</a>
	<h1>New run</h1>
</div>

<form method="POST" use:enhance class="form">
	{#if form?.error}
		<p class="error">{form.error}</p>
	{/if}

	<div class="field">
		<label for="strategy_id">Strategy</label>
		<select id="strategy_id" name="strategy_id" required>
			{#each data.strategies as s}
				<option value={s.id} selected={s.id === data.preselectedStrategyId}>{s.name}</option>
			{/each}
		</select>
	</div>

	<div class="row">
		<div class="field">
			<label for="interval">Interval</label>
			<select id="interval" name="interval" required>
				<option value="1min">1 min</option>
				<option value="5min">5 min</option>
				<option value="15min">15 min</option>
				<option value="25min">25 min</option>
				<option value="60min">60 min</option>
				<option value="day" selected>Day</option>
			</select>
		</div>
	</div>

	<div class="row">
		<div class="field">
			<label for="from_date">From</label>
			<input id="from_date" name="from_date" type="date" required />
		</div>
		<div class="field">
			<label for="to_date">To</label>
			<input id="to_date" name="to_date" type="date" required />
		</div>
	</div>

	<div class="field">
		<label for="instrument-search">Instruments</label>
		<InstrumentSearch onselect={add} inputId="instrument-search" />
		{#if selected.length > 0}
			<ul class="selected-list">
				{#each selected as inst}
					<input type="hidden" name="instruments" value={JSON.stringify(inst)} />
					<li>
						<span class="symbol">{inst.trading_symbol}</span>
						{#if inst.custom_symbol}
							<span class="iname">{inst.custom_symbol}</span>
						{/if}
						<span class="tag">{inst.exchange_segment}</span>
						<button type="button" class="remove" onclick={() => remove(inst)}>×</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="footer">
		<a href="/runs" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Start run</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }

	.back {
		color: var(--text-muted);
		font-size: 0.8rem;
		text-decoration: none;
	}

	.back:hover { color: var(--text); }

	h1 {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 8px 0 0;
	}

	.form {
		display: flex;
		flex-direction: column;
		gap: 20px;
		max-width: 480px;
	}

	.row {
		display: grid;
		gap: 16px;
		grid-template-columns: 1fr 1fr;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	label {
		color: var(--text-muted);
		font-size: 0.8rem;
		font-weight: 500;
	}

	input[type='date'],
	select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}

	input[type='date']:focus,
	select:focus { border-color: var(--accent); }

	.selected-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
	}

	.selected-list li {
		align-items: center;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		display: flex;
		gap: 8px;
		padding: 6px 10px;
	}

	.symbol { font-size: 0.875rem; font-weight: 600; }
	.iname { color: var(--text-muted); font-size: 0.8rem; flex: 1; }
	.tag {
		background: var(--bg);
		border-radius: 4px;
		color: var(--text-muted);
		font-size: 0.7rem;
		padding: 2px 6px;
	}

	.remove {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 1rem;
		line-height: 1;
		margin-left: auto;
		padding: 0 2px;
	}

	.remove:hover { color: var(--red); }

	.footer {
		display: flex;
		gap: 12px;
		justify-content: flex-end;
		padding-top: 8px;
	}

	.btn-primary {
		background: var(--accent);
		border: none;
		border-radius: 6px;
		color: #000;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 20px;
	}

	.btn-primary:hover { background: var(--accent-hover); }

	.btn-secondary {
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		font-size: 0.875rem;
		padding: 8px 20px;
		text-decoration: none;
	}

	.btn-secondary:hover { color: var(--text); }

	.error {
		background: var(--red-bg);
		border-radius: 6px;
		color: var(--red-muted);
		font-size: 0.85rem;
		padding: 10px 14px;
	}
</style>
