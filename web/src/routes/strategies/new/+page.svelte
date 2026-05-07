<script lang="ts">
	import { enhance } from '$app/forms'
	import RustEditor from '$lib/components/RustEditor.svelte'

	let { form } = $props()

	const template = `// prices: &[f64]  — close prices up to current candle
// i: usize        — current index (always >= 1)
// Available: rsi(prices, period), sma(prices, period), ema(prices, period)
// Return: 1 = buy, 2 = sell, 0 = hold

let rsi_vals = rsi(prices, 14);
let prev = rsi_vals[i - 1];
let curr = rsi_vals[i];

if prev.is_nan() || curr.is_nan() {
    return 0;
}

if prev >= 30.0 && curr < 30.0 {
    1
} else if prev <= 70.0 && curr > 70.0 {
    2
} else {
    0
}`

	let code = $state(template)
</script>

<div class="header">
	<a href="/strategies" class="back">← Strategies</a>
	<h1>New strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	{#if form?.error}
		<p class="error">{form.error}</p>
	{/if}

	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" placeholder="e.g. RSI mean reversion" required />
	</div>

	<div class="field">
		<label for="code">Signal logic</label>
		<p class="hint">
			Write the body of your signal function. Use <code>prices</code> (close prices), <code>i</code> (current index).
			Return <code>1</code> to buy, <code>2</code> to sell, <code>0</code> to hold.
		</p>
		<RustEditor value={code} onchange={(v) => (code = v)} />
		<input type="hidden" name="code" value={code} />
	</div>

	<div class="footer">
		<a href="/strategies" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Create</button>
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
		max-width: 680px;
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

	input[type='text'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}

	input[type='text']:focus { border-color: var(--accent); }

	.hint {
		color: var(--text-muted);
		font-size: 0.8rem;
		margin: 0;
	}

	.hint code {
		background: var(--bg-surface);
		border-radius: 3px;
		font-size: 0.75rem;
		padding: 1px 4px;
	}

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
