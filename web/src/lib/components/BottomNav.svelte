<script lang="ts">
	import { page } from '$app/stores'
	import MoreMenu from '$lib/components/MoreMenu.svelte'
	
	let { isAdmin = false, moreExpanded = $bindable(false) } = $props()
	
	let lastPath = $state('')
	
	$effect(() => {
		let path = $page.url.pathname
		if (path !== lastPath && lastPath !== '') {
			moreExpanded = false
		}
		lastPath = path
	})
</script>

{#if moreExpanded}
	<MoreMenu {isAdmin} expanded={moreExpanded} />
{/if}

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
	<a class="tab more-tab" onclick={() => moreExpanded = !moreExpanded}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="1"/><circle cx="12" cy="5" r="1"/><circle cx="12" cy="19" r="1"/></svg>
		<span>More</span>
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

		.tab.more-tab {
			position: relative;
		}

		.tab.more-tab::after {
			content: '';
			position: absolute;
			top: -8px;
			right: 0;
			width: 8px;
			height: 8px;
			background: var(--red);
			border-radius: 50%;
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

	.tab.more-tab.active::after {
		background: var(--accent);
	}
</style>
