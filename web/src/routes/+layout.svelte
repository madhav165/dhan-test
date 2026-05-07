<script lang="ts">
	import '../app.css'
	import { user, brokerConnected } from '$lib/stores/auth'
	import Header from '$lib/components/Header.svelte'
	import Sidebar from '$lib/components/Sidebar.svelte'
	import { page } from '$app/stores'

	let { children, data } = $props()

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
		<Sidebar />
		<main>{@render children()}</main>
	</div>
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
		width: 100%;
		min-height: calc(100vh - 56px);
		background: var(--bg);
		color: var(--text);
	}
</style>
