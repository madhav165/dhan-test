<script lang="ts">
	import { page } from '$app/stores'
</script>

<nav class="bottom-nav">
	<a href="/charts" class="tab" class:active={$page.url.pathname.startsWith('/charts')}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
		<span>Charts</span>
	</a>
	<a href="/strategies" class="tab" class:active={$page.url.pathname.startsWith('/strategies')}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
		<span>Strategies</span>
	</a>
	<a href="/runs" class="tab" class:active={$page.url.pathname.startsWith('/runs')}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><polygon points="5 3 19 12 5 21 5 3"/></svg>
		<span>Runs</span>
	</a>
	<a href="/policies" class="tab" class:active={$page.url.pathname.startsWith('/policies')}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
		<span>Policies</span>
	</a>
	<a href="/profile/alerts" class="tab" class:active={$page.url.pathname.startsWith('/profile')}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
		<span>Alerts</span>
	</a>
</nav>

<style>
	.bottom-nav {
		display: none;
	}

	@media (max-width: 768px) {
		.bottom-nav {
			display: flex;
			position: fixed;
			bottom: 0;
			left: 0;
			right: 0;
			height: 60px;
			background: var(--bg-surface);
			border-top: 1px solid var(--border);
			z-index: 20;
		}
	}

	.tab {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 3px;
		color: var(--text-muted);
		text-decoration: none;
		font-size: 10px;
		transition: color 0.15s;
	}

	.tab svg {
		width: 20px;
		height: 20px;
	}

	.tab.active {
		color: var(--accent);
	}
</style>
