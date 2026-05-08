<script lang="ts">
	import { enhance } from '$app/forms'
	import type { RLConfig, RLConstraint, RLReward } from '$lib/types/rl'
	import type { Indicator } from '$lib/types/rules'

	let { form } = $props()

	const indicatorNames = ['rsi', 'sma', 'ema', 'vwap', 'macd', 'bb'] as const

	function defaultIndicator(name: string): Indicator {
		if (name === 'macd') return { name: 'macd', component: 'macd', fast: 12, slow: 26, signal_period: 9 }
		if (name === 'bb') return { name: 'bb', component: 'upper', period: 20 }
		if (name === 'vwap') return { name: 'vwap' }
		return { name: name as any, period: 14 }
	}

	let reward: RLReward = $state('pnl')
	let allow_short = $state(false)
	let lookback_candles = $state(20)
	let train_from = $state('')
	let train_to = $state('')
	let selectedIndicators: string[] = $state(['rsi', 'sma'])
	let constraints: RLConstraint[] = $state([])

	function toggleIndicator(name: string) {
		if (selectedIndicators.includes(name)) {
			selectedIndicators = selectedIndicators.filter(n => n !== name)
		} else {
			selectedIndicators = [...selectedIndicators, name]
		}
	}

	function addConstraint(type: RLConstraint['type']) {
		const defaults: Record<RLConstraint['type'], RLConstraint> = {
			max_holding_days: { type: 'max_holding_days', value: 5 },
			max_trades_per_month: { type: 'max_trades_per_month', value: 10 },
		}
		constraints = [...constraints, defaults[type]]
	}

	function removeConstraint(i: number) {
		constraints = constraints.filter((_, idx) => idx !== i)
	}

	function updateConstraintValue(i: number, value: number) {
		constraints = constraints.map((c, idx) => idx === i ? { ...c, value } : c)
	}

	let rlConfig = $derived<RLConfig>({
		reward,
		constraints,
		indicators: selectedIndicators.map(defaultIndicator),
		lookback_candles,
		allow_short,
		train_from,
		train_to,
	})
</script>

<div class="header">
	<a href="/strategies/new" class="back">← New strategy</a>
	<h1>Learn strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	{#if form?.error}
		<p class="error">{form.error}</p>
	{/if}

	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" placeholder="e.g. RSI momentum RL" required />
	</div>

	<div class="field">
		<label>Reward objective</label>
		<div class="radio-group">
			{#each [['pnl', 'Maximize PnL'], ['sharpe', 'Maximize Sharpe ratio'], ['min_drawdown', 'Minimize drawdown']] as [val, label]}
				<label class="radio">
					<input type="radio" bind:group={reward} value={val} />
					{label}
				</label>
			{/each}
		</div>
	</div>

	<div class="field">
		<label>Indicators (features)</label>
		<div class="indicator-grid">
			{#each indicatorNames as n}
				<button type="button"
					class="ind-btn"
					class:selected={selectedIndicators.includes(n)}
					onclick={() => toggleIndicator(n)}>
					{n.toUpperCase()}
				</button>
			{/each}
		</div>
		<p class="hint">Selected indicators + a {lookback_candles}-candle OHLCV window form the state.</p>
	</div>

	<div class="field">
		<label>OHLCV lookback candles</label>
		<input type="number" min="5" max="100" bind:value={lookback_candles} class="num-input" />
	</div>

	<div class="field">
		<label>Constraints</label>
		<div class="constraints">
			{#each constraints as c, i}
				<div class="constraint-row">
					<span class="constraint-label">
						{c.type === 'max_holding_days' ? 'Max holding days' : 'Max trades/month'}
					</span>
					<input type="number" min="1" value={c.value}
						oninput={(e) => updateConstraintValue(i, +(e.target as HTMLInputElement).value)}
						class="num-input" />
					<button type="button" class="del" onclick={() => removeConstraint(i)}>✕</button>
				</div>
			{/each}
		</div>
		<div class="constraint-add">
			<button type="button" class="btn-add" onclick={() => addConstraint('max_holding_days')}>+ Max holding days</button>
			<button type="button" class="btn-add" onclick={() => addConstraint('max_trades_per_month')}>+ Max trades/month</button>
		</div>
	</div>

	<div class="field">
		<label class="checkbox-label">
			<input type="checkbox" bind:checked={allow_short} />
			Allow short positions
		</label>
	</div>

	<div class="field">
		<label>Training data range</label>
		<div class="date-row">
			<input type="date" bind:value={train_from} required />
			<span>→</span>
			<input type="date" bind:value={train_to} required />
		</div>
	</div>

	<input type="hidden" name="rl_config" value={JSON.stringify(rlConfig)} />

	<div class="footer">
		<a href="/strategies/new" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Start training</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }
	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }
	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }
	.form { display: flex; flex-direction: column; gap: 20px; max-width: 560px; }
	.field { display: flex; flex-direction: column; gap: 8px; }
	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }
	.hint { color: var(--text-muted); font-size: 0.75rem; margin: 0; }

	input[type='text'], input[type='date'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}
	input[type='text']:focus, input[type='date']:focus { border-color: var(--accent); }

	.num-input {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-size: 0.875rem;
		padding: 6px 10px;
		width: 80px;
	}

	.radio-group { display: flex; flex-direction: column; gap: 8px; }
	.radio { align-items: center; color: var(--text); display: flex; font-size: 0.875rem; font-weight: 400; gap: 8px; }

	.indicator-grid { display: flex; flex-wrap: wrap; gap: 8px; }
	.ind-btn {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.8rem;
		padding: 4px 12px;
	}
	.ind-btn.selected { border-color: var(--accent); color: var(--accent); }

	.constraints { display: flex; flex-direction: column; gap: 8px; }
	.constraint-row { align-items: center; display: flex; gap: 10px; }
	.constraint-label { color: var(--text); font-size: 0.85rem; min-width: 160px; }
	.del { background: none; border: none; color: var(--text-muted); cursor: pointer; font-size: 0.75rem; }
	.del:hover { color: var(--red-muted); }

	.constraint-add { display: flex; gap: 8px; }
	.btn-add {
		background: none;
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.8rem;
		padding: 4px 10px;
	}
	.btn-add:hover { border-color: var(--accent); color: var(--accent); }

	.checkbox-label { align-items: center; color: var(--text); display: flex; font-size: 0.875rem; font-weight: 400; gap: 8px; }

	.date-row { align-items: center; display: flex; gap: 10px; }
	.date-row span { color: var(--text-muted); }

	.footer { display: flex; gap: 12px; justify-content: flex-end; padding-top: 8px; }
	.btn-primary {
		background: var(--accent); border: none; border-radius: 6px; color: #000;
		cursor: pointer; font-family: 'Inter', sans-serif; font-size: 0.875rem;
		font-weight: 500; padding: 8px 20px;
	}
	.btn-primary:hover { background: var(--accent-hover); }
	.btn-secondary {
		border: 1px solid var(--border); border-radius: 6px; color: var(--text-muted);
		font-size: 0.875rem; padding: 8px 20px; text-decoration: none;
	}
	.btn-secondary:hover { color: var(--text); }
	.error { background: var(--red-bg); border-radius: 6px; color: var(--red-muted); font-size: 0.85rem; padding: 10px 14px; }
</style>
