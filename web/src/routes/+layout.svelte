<script lang="ts">
	import '../app.css'
	import { user, brokerConnected } from '$lib/stores/auth'
	import Header from '$lib/components/Header.svelte'
	import Sidebar from '$lib/components/Sidebar.svelte'
	import BottomNav from '$lib/components/BottomNav.svelte'
	import { page } from '$app/stores'

	let { children, data } = $props()

	let moreExpanded = $state(false)

	$effect(() => {
		user.set(data.user ?? null)
		brokerConnected.set(data.brokerConnected ?? false)
	})

	const isAuthRoute = $derived(
		$page.url.pathname.startsWith('/auth') || $page.url.pathname.startsWith('/login')
	)
</script>

{#if !isAuthRoute && $user}
	<Header />
	<div class="shell">
		<Sidebar isAdmin={data.isAdmin ?? false} />
		<main class:full-bleed={$page.url.pathname.startsWith('/charts')}>{@render children()}</main>
	</div>
	<BottomNav isAdmin={data.isAdmin ?? false} moreExpanded={moreExpanded} />
{:else}
	{@render children()}
{/if}

<style>
	.shell {
		display: flex;
		padding-top: 56px;
	}

	main {
		margin-left: 200px;
		padding: 32px;
		width: calc(100% - 200px);
		height: calc(100vh - 56px);
		overflow-y: auto;
		background: var(--bg);
		color: var(--text);
		box-sizing: border-box;
	}

	main.full-bleed {
		padding: 0;
		overflow: hidden;
	}

	@media (max-width: 768px) {
		main {
			margin-left: 0;
			width: 100%;
			padding: 20px 16px 80px;
			height: calc(100vh - 56px);
		}

		main.full-bleed {
			padding: 0 0 60px;
		}
	}
</style>
