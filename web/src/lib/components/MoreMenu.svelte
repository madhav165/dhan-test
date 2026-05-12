<script lang="ts">
	import { page } from '$app/stores'
	
	let { isAdmin = false, expanded = $bindable(false) } = $props()
</script>

<div class="more-menu" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="More options" tabindex="0" onkeydown={(e) => { if (e.key === 'Escape') expanded = false }}>
	<div class="menu-header">
		<h3>More Options</h3>
		<button class="close-btn" onclick={() => expanded = false}>✕</button>
	</div>

	<nav class="menu-nav">
		<a href="/profile/alerts" class="nav-link" class:active={$page.url.pathname.startsWith('/profile/alerts')}>
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
			<span>Alerts</span>
		</a>
		<a href="/ohlcv" class="nav-link" class:active={$page.url.pathname.startsWith('/ohlcv')}>
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="4" y="4" width="4" height="16"/><rect x="12" y="4" width="4" height="10"/><rect x="20" y="4" width="4" height="14"/></svg>
			<span>OHLCV</span>
		</a>
		{#if isAdmin}
			<a href="/admin" class="nav-link" class:active={$page.url.pathname.startsWith('/admin')}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>
				<span>Admin</span>
			</a>
		{/if}
	</nav>

	</div>

<style>
	.more-menu {
		position: fixed;
		top: 56px;
		bottom: 60px;
		left: 0;
		right: 0;
		background: var(--bg);
		z-index: 30;
		overflow-y: auto;
	}

	@media (min-width: 769px) {
		.more-menu { display: none; }
	}

	.menu-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px;
		border-bottom: 1px solid var(--border);
	}

	.menu-header h3 {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
	}

	.close-btn {
		background: none;
		border: none;
		font-size: 20px;
		color: var(--text-muted);
		cursor: pointer;
		padding: 4px 8px;
	}

	.menu-nav {
		padding: 16px 0;
	}

	.nav-link {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-muted);
		text-decoration: none;
		padding: 12px 16px;
		transition: background 0.15s, color 0.15s;
	}

	.nav-link:hover {
		background: var(--bg-surface);
		color: var(--text);
	}

	.nav-link.active {
		background: var(--bg-surface);
		color: var(--text);
		font-weight: 500;
	}

	.nav-link svg {
		width: 20px;
		height: 20px;
	}

	
</style>
