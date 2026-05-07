<div class="container">
	<div class="card">
		<h1>Dhan</h1>
		<p>Sign in to continue</p>
		<a href="/auth/google" class="btn">Sign in with Google</a>
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
		margin: 0 0 32px;
	}

	.btn {
		display: block;
		background: var(--green);
		color: #fff;
		text-decoration: none;
		padding: 12px 24px;
		border-radius: 8px;
		font-size: 15px;
		font-weight: 500;
		transition: background 0.15s;
	}

	.btn:hover {
		background: var(--green-hover);
	}
</style>
