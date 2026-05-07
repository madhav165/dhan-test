<script lang="ts">
	import { page } from '$app/stores'
	const error = $derived($page.url.searchParams.get('error'))
</script>

<div class="container">
	<div class="card">
		<h1>Dhan</h1>
		<p>Enter your Dhan Client ID to connect</p>
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
			<button type="submit" class="btn">Login with Dhan</button>
		</form>
	</div>
</div>

<style>
	.container {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #0f172a;
	}

	.card {
		background: #1e293b;
		border: 1px solid #334155;
		border-radius: 12px;
		padding: 48px;
		text-align: center;
		width: 100%;
		max-width: 360px;
	}

	h1 {
		color: #f1f5f9;
		font-size: 28px;
		font-weight: 700;
		margin: 0 0 8px;
	}

	p {
		color: #64748b;
		font-size: 14px;
		margin: 0 0 24px;
	}

	.error {
		color: #f87171;
		font-size: 13px;
		margin: -12px 0 16px;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	input {
		background: #0f172a;
		border: 1px solid #334155;
		border-radius: 8px;
		color: #f1f5f9;
		font-family: 'Inter', sans-serif;
		font-size: 15px;
		padding: 12px 16px;
		text-align: center;
		letter-spacing: 2px;
		outline: none;
		width: 100%;
	}

	input:focus {
		border-color: #00c278;
	}

	input::placeholder {
		color: #475569;
		letter-spacing: normal;
	}

	.btn {
		background: #00c278;
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
		background: #00a866;
	}
</style>
