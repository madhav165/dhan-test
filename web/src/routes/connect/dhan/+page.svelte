<script lang="ts">
	import { page } from '$app/stores'
	const error = $derived($page.url.searchParams.get('error'))
</script>

<div class="container">
	<div class="card">
		<h1>Connect Dhan</h1>
		<p>Enter your 10-digit Dhan Client ID to link your account</p>
		{#if error === 'invalid_client_id'}
			<p class="error">Please enter a valid 10-digit Dhan Client ID</p>
		{/if}
		<form method="GET" action="/auth/dhan">
			<input
				type="text"
				name="client_id"
				placeholder="10-digit Client ID"
				maxlength="10"
				required
			/>
			<button type="submit" class="btn">Connect</button>
		</form>
	</div>
</div>

<style>
	.container {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--bg);
	}

	.card {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 12px;
		padding: 48px;
		text-align: center;
		width: 100%;
		max-width: 360px;
	}

	h1 {
		color: var(--text);
		font-size: 28px;
		font-weight: 700;
		margin: 0 0 8px;
	}

	p {
		color: var(--text-muted);
		font-size: 14px;
		margin: 0 0 24px;
	}

	.error {
		color: var(--red);
		font-size: 13px;
		margin: -12px 0 16px;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	input {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 8px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 15px;
		padding: 12px 16px;
		text-align: center;
		letter-spacing: 2px;
		outline: none;
		width: 100%;
	}

	input:focus {
		border-color: var(--green);
	}

	input::placeholder {
		color: var(--text-faint);
		letter-spacing: normal;
	}

	.btn {
		background: var(--green);
		border: none;
		border-radius: 8px;
		color: #fff;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 15px;
		font-weight: 500;
		padding: 12px 24px;
		transition: background 0.15s;
		width: 100%;
	}

	.btn:hover {
		background: var(--green-hover);
	}
</style>
