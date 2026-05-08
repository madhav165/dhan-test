<script lang="ts">
	import { enhance } from '$app/forms'
	import InstrumentSearch from '$lib/components/InstrumentSearch.svelte'

	let { data } = $props()

	type Instrument = {
		security_id: string
		exchange_segment: string
		trading_symbol: string
		custom_symbol: string
	}

	let selected = $state<Instrument[]>(data.policy.instruments ?? [])

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
	<a href="/policies/{data.policy.id}" class="back">← Policy</a>
	<h1>Edit policy</h1>
</div>

<form method="POST" use:enhance class="form">
	<div class="field">
		<label>Mode</label>
		<div class="mode-options">
			<label class="mode-option">
				<input type="radio" name="mode" value="alert" checked={data.policy.mode === 'alert'} />
				<span>
					<strong>Alert</strong>
					<small>Notify when a signal fires</small>
				</span>
			</label>
			<label class="mode-option">
				<input type="radio" name="mode" value="trade" checked={data.policy.mode === 'trade'} />
				<span>
					<strong>Trade</strong>
					<small>Place orders automatically via Dhan</small>
				</span>
			</label>
		</div>
	</div>

	<div class="field">
		<label for="interval">Interval</label>
		<select id="interval" name="interval" required>
			{#each ['1min','5min','15min','25min','60min','day'] as iv}
				<option value={iv} selected={data.policy.interval === iv}>{iv}</option>
			{/each}
		</select>
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
						<span class="tag">{inst.exchange_segment}</span>
						<button type="button" class="remove" onclick={() => remove(inst)}>×</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="footer">
		<a href="/policies/{data.policy.id}" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Save</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }

	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }

	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }

	.form { display: flex; flex-direction: column; gap: 20px; max-width: 480px; }

	.field { display: flex; flex-direction: column; gap: 6px; }

	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }

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

	select:focus { border-color: var(--accent); }

	.mode-options { display: flex; flex-direction: column; gap: 8px; }

	.mode-option {
		align-items: flex-start;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		cursor: pointer;
		display: flex;
		gap: 12px;
		padding: 12px 14px;
		transition: border-color 0.15s;
	}

	.mode-option:has(input:checked) { border-color: var(--accent); }
	.mode-option input { margin-top: 2px; }
	.mode-option span { display: flex; flex-direction: column; gap: 2px; }
	.mode-option strong { color: var(--text); font-size: 0.875rem; }
	.mode-option small { color: var(--text-muted); font-size: 0.75rem; }

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

	.symbol { font-size: 0.875rem; font-weight: 600; flex: 1; }
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
		padding: 0 2px;
	}

	.remove:hover { color: var(--red); }

	.footer { display: flex; gap: 12px; justify-content: flex-end; padding-top: 8px; }

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
</style>
