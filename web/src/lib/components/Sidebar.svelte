<script lang="ts">
	import { brokerConnected } from '$lib/stores/auth'
</script>

<aside>
	<div class="section">
		<p class="label">Brokers</p>
		<a href={$brokerConnected ? '#' : '/connect/dhan'} class="broker" class:connected={$brokerConnected}>
			<span class="dot"></span>
			Dhan
		</a>
		<hr />
	</div>
</aside>

<style>
	aside {
		background: var(--bg-surface);
		border-right: 1px solid var(--border);
		bottom: 0;
		left: 0;
		padding: 24px 16px;
		position: fixed;
		top: 56px;
		width: 200px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.label {
		color: var(--text-faint);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.08em;
		margin: 0 0 8px;
		text-transform: uppercase;
	}

	.broker {
		align-items: center;
		border-radius: 6px;
		color: var(--red);
		display: flex;
		font-size: 14px;
		gap: 8px;
		padding: 8px 10px;
		text-decoration: none;
		transition: background 0.15s;
	}

	.broker:hover {
		background: var(--bg);
	}

	.broker.connected {
		color: var(--text);
	}

	.dot {
		background: var(--red);
		border-radius: 50%;
		flex-shrink: 0;
		height: 7px;
		width: 7px;
	}

	.broker.connected .dot {
		background: var(--green);
	}

	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 8px 0;
	}
</style>
