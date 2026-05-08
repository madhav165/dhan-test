<script lang="ts">
	import { user, brokerConnected } from '$lib/stores/auth'
	import { invalidateAll } from '$app/navigation'
	import { messages } from '$lib/messages'

	let expanded = $state(false)
	let clientId = $state('')
	let error = $state('')

	async function disconnect() {
		await fetch('/auth/dhan/disconnect', { method: 'POST' })
		await invalidateAll()
		expanded = false
	}

	function validate() {
		if (!/^\d{10}$/.test(clientId.trim())) {
			error = messages.broker.invalidClientId
			return false
		}
		error = ''
		return true
	}
</script>

<header>
	<a href="/charts" class="logo">Dhan</a>
	<div class="right">
		<!-- broker button: mobile only -->
		<div class="broker-wrap mobile-only">
			<button class="broker" class:connected={$brokerConnected} onclick={() => expanded = !expanded}>
				<span class="dot"></span>
				Dhan
			</button>
			{#if expanded}
				<div class="popover">
					{#if $brokerConnected}
						<button class="disconnect-btn" onclick={disconnect}>Disconnect</button>
					{:else}
						<form method="GET" action="/auth/dhan" onsubmit={(e) => { if (!validate()) e.preventDefault() }}>
							<input type="text" name="client_id" bind:value={clientId} placeholder="10-digit Client ID" maxlength="10" />
							{#if error}<p class="error">{error}</p>{/if}
							<button type="submit" class="connect-btn">Connect</button>
						</form>
					{/if}
				</div>
			{/if}
		</div>

		<div class="user">
			<span class="name desktop-only">{$user?.name}</span>
			<form method="POST" action="/auth/logout">
				<button type="submit" title="Logout">↪</button>
			</form>
		</div>
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

	.right {
		display: flex;
		align-items: center;
		gap: 12px;
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

	/* broker button */
	.broker-wrap {
		position: relative;
	}

	.broker {
		align-items: center;
		display: flex;
		gap: 6px;
		color: var(--red);
		font-size: 13px;
		border-color: transparent;
	}

	.broker.connected {
		color: var(--text);
	}

	.dot {
		background: var(--red);
		border-radius: 50%;
		height: 7px;
		width: 7px;
		flex-shrink: 0;
	}

	.broker.connected .dot {
		background: var(--green);
	}

	.popover {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 12px;
		position: absolute;
		right: 0;
		top: calc(100% + 8px);
		width: 200px;
		z-index: 100;
	}

	.popover form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.popover input[type="text"] {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 13px;
		outline: none;
		padding: 8px 10px;
		text-align: center;
		letter-spacing: 1px;
		width: 100%;
	}

	.error {
		color: var(--red);
		font-size: 11px;
		margin: 0;
	}

	.connect-btn {
		background: none;
		border: 1px solid var(--accent);
		border-radius: 6px;
		color: var(--accent);
		cursor: pointer;
		font-size: 13px;
		font-weight: 500;
		padding: 8px;
		width: 100%;
	}

	.connect-btn:hover {
		background: var(--accent);
		color: #fff;
	}

	.disconnect-btn {
		background: none;
		border: 1px solid var(--red);
		border-radius: 6px;
		color: var(--red);
		cursor: pointer;
		font-size: 13px;
		font-weight: 500;
		padding: 8px;
		width: 100%;
	}

	.disconnect-btn:hover {
		background: var(--red);
		color: #fff;
	}

	.mobile-only { display: none; }
	.desktop-only { display: inline; }

	@media (max-width: 768px) {
		.mobile-only { display: block; }
		.desktop-only { display: none; }
	}
</style>
