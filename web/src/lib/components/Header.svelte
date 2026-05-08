<script lang="ts">
	import { user } from '$lib/stores/auth'
</script>

<header>
	<a href="/charts" class="logo">Dhan</a>
	<div class="user">
		<span class="name">{$user?.name}</span>
		<form method="POST" action="/auth/logout">
			<button type="submit" title="Logout">↪</button>
		</form>
	</div>
</header>

<style>
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		background: var(--bg-surface);
		border-bottom: 1px solid var(--border);
		height: 56px;
		padding: 0 24px;
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 10;
	}

	.logo {
		color: var(--accent);
		font-size: 18px;
		font-weight: 700;
		text-decoration: none;
	}

	.user {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.name {
		color: var(--text-subtle);
		font-size: 14px;
	}

	button {
		background: none;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 14px;
		padding: 4px 8px;
		transition: color 0.15s, border-color 0.15s;
	}

	button:hover {
		border-color: var(--text-muted);
		color: var(--text-subtle);
	}
</style>
