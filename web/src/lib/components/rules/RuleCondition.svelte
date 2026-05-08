<script lang="ts">
	import type { Condition, Indicator, Operand, Operator } from '$lib/types/rules'

	let { condition, onchange, ondelete }: {
		condition: Condition
		onchange: (c: Condition) => void
		ondelete: () => void
	} = $props()

	const rawInputs = ['price', 'volume'] as const
	const indicatorNames = ['rsi', 'sma', 'ema', 'vwap', 'macd', 'bb'] as const
	const operators: Operator[] = ['>', '<', '>=', '<=', '==', 'crosses_above', 'crosses_below']
	const macdComponents = ['macd', 'signal', 'histogram'] as const
	const bbComponents = ['upper', 'middle', 'lower'] as const

	function operandKind(op: Operand): string {
		if (op.kind === 'raw') return `raw:${op.input.kind}`
		if (op.kind === 'number') return 'number'
		return `ind:${op.indicator.name}`
	}

	function defaultIndicator(name: string): Indicator {
		if (name === 'macd') return { name: 'macd', component: 'macd', fast: 12, slow: 26, signal_period: 9 }
		if (name === 'bb') return { name: 'bb', component: 'upper', period: 20 }
		if (name === 'vwap') return { name: 'vwap' }
		return { name: name as any, period: 14 }
	}

	function setOperandFromKey(key: string, side: 'left' | 'right') {
		let op: Operand
		if (key === 'raw:price') op = { kind: 'raw', input: { kind: 'price' } }
		else if (key === 'raw:volume') op = { kind: 'raw', input: { kind: 'volume' } }
		else if (key === 'number') op = { kind: 'number', value: 0 }
		else op = { kind: 'indicator', indicator: defaultIndicator(key.replace('ind:', '')) }
		onchange({ ...condition, [side]: op })
	}

	function patchIndicator(side: 'left' | 'right', patch: Partial<Indicator>) {
		const op = condition[side]
		if (op.kind !== 'indicator') return
		onchange({ ...condition, [side]: { kind: 'indicator', indicator: { ...op.indicator, ...patch } as Indicator } })
	}
</script>

<div class="condition">
	<!-- Left operand -->
	<select value={operandKind(condition.left)}
		onchange={(e) => setOperandFromKey((e.target as HTMLSelectElement).value, 'left')}>
		<optgroup label="Raw">
			{#each rawInputs as r}<option value="raw:{r}">{r}</option>{/each}
		</optgroup>
		<optgroup label="Indicators">
			{#each indicatorNames as n}<option value="ind:{n}">{n.toUpperCase()}</option>{/each}
		</optgroup>
	</select>

	{#if condition.left.kind === 'indicator'}
		{#if condition.left.indicator.name === 'macd'}
			<select value={condition.left.indicator.component}
				onchange={(e) => patchIndicator('left', { component: (e.target as HTMLSelectElement).value as any })}>
				{#each macdComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.left.indicator.fast} min="1" class="num-sm" placeholder="fast"
				oninput={(e) => patchIndicator('left', { fast: +(e.target as HTMLInputElement).value })} />
			<input type="number" value={condition.left.indicator.slow} min="1" class="num-sm" placeholder="slow"
				oninput={(e) => patchIndicator('left', { slow: +(e.target as HTMLInputElement).value })} />
			<input type="number" value={condition.left.indicator.signal_period} min="1" class="num-sm" placeholder="sig"
				oninput={(e) => patchIndicator('left', { signal_period: +(e.target as HTMLInputElement).value })} />
		{:else if condition.left.indicator.name === 'bb'}
			<select value={condition.left.indicator.component}
				onchange={(e) => patchIndicator('left', { component: (e.target as HTMLSelectElement).value as any })}>
				{#each bbComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.left.indicator.period} min="1" class="num-sm"
				oninput={(e) => patchIndicator('left', { period: +(e.target as HTMLInputElement).value })} />
		{:else if condition.left.indicator.name !== 'vwap'}
			<input type="number" value={condition.left.indicator.period} min="1" class="num-sm"
				oninput={(e) => patchIndicator('left', { period: +(e.target as HTMLInputElement).value })} />
		{/if}
	{/if}

	<!-- Operator -->
	<select value={condition.operator}
		onchange={(e) => onchange({ ...condition, operator: (e.target as HTMLSelectElement).value as Operator })}>
		{#each operators as op}<option value={op}>{op}</option>{/each}
	</select>

	<!-- Right operand -->
	<select value={operandKind(condition.right)}
		onchange={(e) => setOperandFromKey((e.target as HTMLSelectElement).value, 'right')}>
		<option value="number">Number</option>
		<optgroup label="Raw">
			{#each rawInputs as r}<option value="raw:{r}">{r}</option>{/each}
		</optgroup>
		<optgroup label="Indicators">
			{#each indicatorNames as n}<option value="ind:{n}">{n.toUpperCase()}</option>{/each}
		</optgroup>
	</select>

	{#if condition.right.kind === 'number'}
		<input type="number" value={condition.right.value} class="num-md"
			oninput={(e) => onchange({ ...condition, right: { kind: 'number', value: +(e.target as HTMLInputElement).value } })} />
	{:else if condition.right.kind === 'indicator'}
		{#if condition.right.indicator.name === 'macd'}
			<select value={condition.right.indicator.component}
				onchange={(e) => patchIndicator('right', { component: (e.target as HTMLSelectElement).value as any })}>
				{#each macdComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.right.indicator.fast} min="1" class="num-sm"
				oninput={(e) => patchIndicator('right', { fast: +(e.target as HTMLInputElement).value })} />
			<input type="number" value={condition.right.indicator.slow} min="1" class="num-sm"
				oninput={(e) => patchIndicator('right', { slow: +(e.target as HTMLInputElement).value })} />
			<input type="number" value={condition.right.indicator.signal_period} min="1" class="num-sm"
				oninput={(e) => patchIndicator('right', { signal_period: +(e.target as HTMLInputElement).value })} />
		{:else if condition.right.indicator.name === 'bb'}
			<select value={condition.right.indicator.component}
				onchange={(e) => patchIndicator('right', { component: (e.target as HTMLSelectElement).value as any })}>
				{#each bbComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.right.indicator.period} min="1" class="num-sm"
				oninput={(e) => patchIndicator('right', { period: +(e.target as HTMLInputElement).value })} />
		{:else if condition.right.indicator.name !== 'vwap'}
			<input type="number" value={condition.right.indicator.period} min="1" class="num-sm"
				oninput={(e) => patchIndicator('right', { period: +(e.target as HTMLInputElement).value })} />
		{/if}
	{/if}

	<button type="button" class="del" onclick={ondelete}>✕</button>
</div>

<style>
	.condition {
		align-items: center;
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	select, input[type='number'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text);
		font-size: 0.8rem;
		padding: 4px 6px;
	}

	.num-sm { width: 56px; }
	.num-md { width: 80px; }

	.del {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.75rem;
		padding: 2px 4px;
	}

	.del:hover { color: var(--red-muted); }
</style>
