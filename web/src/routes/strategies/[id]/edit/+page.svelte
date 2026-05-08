<script lang="ts">
	import { enhance } from '$app/forms'
	import RustEditor from '$lib/components/RustEditor.svelte'

	let { data } = $props()

	let code = $state(data.strategy.code ?? '')
	let name = $state(data.strategy.name)
</script>

<div class="header">
	<a href="/strategies/{data.strategy.id}" class="back">← {data.strategy.name}</a>
	<h1>Edit strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" bind:value={name} required />
	</div>

	<div class="field">
		<label for="code">Signal logic</label>
		<p class="hint">
			Saving will re-compile and replace the existing WASM. Use <code>prices</code> (close prices), <code>i</code> (current index).
			Return <code>1</code> to buy, <code>2</code> to sell, <code>0</code> to hold.
		</p>
		<RustEditor value={code} onchange={(v) => (code = v)} />
		<input type="hidden" name="code" value={code} />
	</div>

	<div class="footer">
		<a href="/strategies/{data.strategy.id}" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Save & rebuild</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }

	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }

	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }

	.form { display: flex; flex-direction: column; gap: 20px; max-width: 680px; }

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

	.hint { color: var(--text-muted); font-size: 0.8rem; margin: 0; }
	.hint code { background: var(--bg-surface); border-radius: 3px; font-size: 0.75rem; padding: 1px 4px; }

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
