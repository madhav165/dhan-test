<script lang="ts">
	import { enhance } from '$app/forms'
	import InstrumentSearch from '$lib/components/InstrumentSearch.svelte'
	import type { RLConfig, RLConstraint, RLReward } from '$lib/types/rl'
	import type { Indicator } from '$lib/types/rules'

	let { form } = $props()

	type Instrument = { security_id: string; exchange_segment: string; trading_symbol: string; custom_symbol: string }
	let instrument = $state<Instrument | null>(null)

	const indicatorNames = ['rsi', 'sma', 'ema', 'wma', 'vwap', 'macd', 'bb', 'atr', 'stoch', 'obv', 'cci'] as const

	function defaultIndicator(name: string): Indicator {
		if (name === 'macd') return { name: 'macd', component: 'macd', fast: 12, slow: 26, signal_period: 9 }
		if (name === 'bb') return { name: 'bb', component: 'upper', period: 20 }
		if (name === 'vwap') return { name: 'vwap' }
		if (name === 'obv') return { name: 'obv' }
		return { name: name as any, period: 14 }
	}

	let reward: RLReward = $state('pnl')
	let allow_short = $state(false)
	let lookback_candles = $state(20)
	let train_from = $state('')
	let train_to = $state('')
	let selectedIndicators: string[] = $state(['rsi', 'sma'])
	let constraints: RLConstraint[] = $state([])
	let training_method = $state<'ppo' | 'reinforce'>('ppo')
	let lr = $state(0.0001)
	let hidden_size = $state(64)
	let ppo_epochs = $state(4)
	let clip_epsilon = $state(0.2)
	let value_coef = $state(0.5)
	let entropy_coef = $state(0.01)
	let gae_lambda = $state(0.95)
	let batch_episodes = $state(8)

	function fmtDate(d: Date) {
		return d.toISOString().slice(0, 10)
	}

	function splitPreview(from: string, to: string) {
		if (!from || !to || from > to) return null
		const start = new Date(`${from}T00:00:00Z`)
		const end = new Date(`${to}T00:00:00Z`)
		const days = Math.floor((end.getTime() - start.getTime()) / 86400000) + 1
		if (days < 3) return null
		const trainDays = Math.max(1, Math.floor(days * 0.7))
		const valDays = Math.max(1, Math.floor(days * 0.15))
		const trainEnd = new Date(start); trainEnd.setUTCDate(start.getUTCDate() + trainDays - 1)
		const valStart = new Date(trainEnd); valStart.setUTCDate(trainEnd.getUTCDate() + 1)
		const valEnd = new Date(valStart); valEnd.setUTCDate(valStart.getUTCDate() + valDays - 1)
		const testStart = new Date(valEnd); testStart.setUTCDate(valEnd.getUTCDate() + 1)
		return {
			train: `${fmtDate(start)} → ${fmtDate(trainEnd)}`,
			val: `${fmtDate(valStart)} → ${fmtDate(valEnd)}`,
			test: `${fmtDate(testStart)} → ${fmtDate(end)}`,
		}
	}

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
		security_id: instrument?.security_id ?? '',
		exchange_segment: instrument?.exchange_segment ?? '',
		trading_symbol: instrument?.trading_symbol ?? '',
		train_from,
		train_to,
		training_method,
		lr,
		hidden_size,
		ppo_epochs,
		clip_epsilon,
		value_coef,
		entropy_coef,
		gae_lambda,
		batch_episodes,
	})

	let split = $derived(splitPreview(train_from, train_to))
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
		<label>Instrument</label>
		<InstrumentSearch onselect={(inst) => { instrument = inst }} inputId="rl-instrument-search" />
		{#if instrument}
			<div class="selected-inst">
				<span class="symbol">{instrument.trading_symbol}</span>
				{#if instrument.custom_symbol}<span class="iname">{instrument.custom_symbol}</span>{/if}
				<span class="tag">{instrument.exchange_segment}</span>
				<button type="button" class="remove" onclick={() => { instrument = null }}>×</button>
			</div>
		{/if}
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
		<label>Training method</label>
		<select bind:value={training_method} class="method-select">
			<option value="ppo">PPO (Proximal Policy Optimization)</option>
			<option value="reinforce">REINFORCE (vanilla policy gradient)</option>
		</select>
	</div>

	{#if training_method === 'ppo'}
		<div class="field ppo-params">
			<label>PPO hyperparameters</label>
			<div class="param-grid">
				<div class="param">
					<span class="param-label">LR</span>
					<input type="number" min="0.000001" max="0.01" step="0.000001" bind:value={lr} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">Hidden</span>
					<select bind:value={hidden_size} class="method-select">
						<option value={32}>32</option>
						<option value={64}>64</option>
						<option value={128}>128</option>
						<option value={256}>256</option>
					</select>
				</div>
				<div class="param">
					<span class="param-label">Epochs</span>
					<input type="number" min="1" max="20" bind:value={ppo_epochs} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">Clip ε</span>
					<input type="number" min="0.05" max="0.5" step="0.05" bind:value={clip_epsilon} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">Value coef</span>
					<input type="number" min="0.1" max="2" step="0.1" bind:value={value_coef} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">Entropy coef</span>
					<input type="number" min="0" max="0.1" step="0.005" bind:value={entropy_coef} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">GAE λ</span>
					<input type="number" min="0.8" max="1" step="0.05" bind:value={gae_lambda} class="num-input" />
				</div>
				<div class="param">
					<span class="param-label">Batch episodes</span>
					<input type="number" min="2" max="32" bind:value={batch_episodes} class="num-input" />
				</div>
			</div>
		</div>
	{/if}

	<div class="field">
		<label>Learning data range <span class="label-note">(split automatically into train / validation / test)</span></label>
		<div class="date-row">
			<input type="date" bind:value={train_from} required />
			<span>→</span>
			<input type="date" bind:value={train_to} required />
		</div>
		{#if split}
			<div class="split-preview">
				<span>Train {split.train}</span>
				<span>Validation {split.val}</span>
				<span>Test {split.test}</span>
			</div>
		{/if}
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
	.label-note { font-size: 0.7rem; font-weight: 400; opacity: 0.7; }

	.selected-inst {
		align-items: center;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		display: flex;
		gap: 8px;
		margin-top: 6px;
		padding: 6px 10px;
	}
	.symbol { font-size: 0.875rem; font-weight: 600; }
	.iname { color: var(--text-muted); flex: 1; font-size: 0.8rem; }
	.tag { background: var(--bg); border-radius: 4px; color: var(--text-muted); font-size: 0.7rem; padding: 2px 6px; }
	.remove { background: none; border: none; color: var(--text-muted); cursor: pointer; font-size: 1rem; line-height: 1; margin-left: auto; padding: 0 2px; }
	.remove:hover { color: var(--red); }

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
	.split-preview { color: var(--text-muted); display: flex; flex-direction: column; font-size: 0.75rem; gap: 4px; }

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
	.method-select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		padding: 8px 10px;
	}
	.ppo-params { margin-top: -8px; }
	.param-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
	.param { display: flex; flex-direction: column; gap: 4px; }
	.param-label { color: var(--text-muted); font-size: 0.7rem; }
	.error { background: var(--red-bg); border-radius: 6px; color: var(--red-muted); font-size: 0.85rem; padding: 10px 14px; }
</style>
