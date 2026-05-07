<script lang="ts">
	import { brokerConnected } from '$lib/stores/auth'
</script>

{#if !$brokerConnected}
	<div class="banner">
		Broker disconnected — <a href="/connect/dhan">Connect Dhan</a> to start trading.
	</div>
{/if}

<style>
	.banner {
		background: var(--red-bg);
		border: 1px solid var(--red);
		border-radius: 8px;
		color: var(--red);
		font-size: 14px;
		padding: 12px 16px;
	}

	a {
		color: var(--red-muted);
		font-weight: 500;
	}
</style>
