<script lang="ts">
	import { enhance } from '$app/forms'
	import RuleBuilder from '$lib/components/rules/RuleBuilder.svelte'

	let { form } = $props()
</script>

<div class="header">
	<a href="/strategies/new" class="back">← New strategy</a>
	<h1>Define rules</h1>
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
		<label>Signal rules</label>
		<RuleBuilder />
	</div>

	<div class="footer">
		<a href="/strategies" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Create</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }

	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }

	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }

	.form { display: flex; flex-direction: column; gap: 20px; max-width: 720px; }

	.field { display: flex; flex-direction: column; gap: 6px; }

	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }

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

	.error {
		background: var(--red-bg);
		border-radius: 6px;
		color: var(--red-muted);
		font-size: 0.85rem;
		padding: 10px 14px;
	}
</style>
