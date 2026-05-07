<script lang="ts">
	type Instrument = {
		security_id: string
		exchange_segment: string
		trading_symbol: string
		custom_symbol: string
		instrument_type: string
	}

	type Props = {
		onselect: (instrument: Instrument) => void
		placeholder?: string
	}

	let { onselect, placeholder = 'Search instruments…' }: Props = $props()

	let query = $state('')
	let results = $state<Instrument[]>([])
	let loading = $state(false)
	let open = $state(false)
	let debounceTimer: ReturnType<typeof setTimeout>

	function onInput() {
		clearTimeout(debounceTimer)
		if (query.length < 1) {
			results = []
			open = false
			return
		}
		debounceTimer = setTimeout(async () => {
			loading = true
			try {
				const resp = await fetch(`/api/instruments/search?q=${encodeURIComponent(query)}`)
				results = await resp.json()
				open = results.length > 0
			} finally {
				loading = false
			}
		}, 250)
	}

	function select(instrument: Instrument) {
		query = instrument.trading_symbol
		open = false
		onselect(instrument)
	}

	function onBlur() {
		setTimeout(() => { open = false }, 150)
	}
</script>

<div class="search-wrapper">
	<input
		type="text"
		bind:value={query}
		oninput={onInput}
		onblur={onBlur}
		onfocus={() => { if (results.length > 0) open = true }}
		{placeholder}
		autocomplete="off"
	/>
	{#if loading}
		<span class="hint">Searching…</span>
	{/if}
	{#if open}
		<ul class="dropdown">
			{#each results as inst}
				<li>
					<button type="button" onclick={() => select(inst)}>
						<span class="symbol">{inst.trading_symbol}</span>
						{#if inst.custom_symbol}
							<span class="name">{inst.custom_symbol}</span>
						{/if}
						<span class="tag">{inst.exchange_segment}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.search-wrapper {
		position: relative;
		width: 100%;
	}

	input {
		width: 100%;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--bg);
		color: var(--text);
		font-size: 0.9rem;
		box-sizing: border-box;
	}

	input:focus {
		outline: none;
		border-color: var(--accent);
	}

	.hint {
		position: absolute;
		right: 0.75rem;
		top: 50%;
		transform: translateY(-50%);
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	.dropdown {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		right: 0;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		list-style: none;
		margin: 0;
		padding: 0.25rem 0;
		z-index: 50;
		max-height: 300px;
		overflow-y: auto;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
	}

	.dropdown li button {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
		color: var(--text);
		font-size: 0.875rem;
	}

	.dropdown li button:hover {
		background: var(--bg-surface);
	}

	.symbol {
		font-weight: 600;
		flex-shrink: 0;
	}

	.name {
		color: var(--text-muted);
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.tag {
		font-size: 0.7rem;
		color: var(--text-muted);
		background: var(--bg-surface);
		padding: 0.1rem 0.4rem;
		border-radius: 4px;
		flex-shrink: 0;
	}
</style>
