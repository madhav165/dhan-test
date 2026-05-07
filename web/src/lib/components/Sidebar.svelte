<script lang="ts">
	import { brokerConnected } from '$lib/stores/auth'
	import { page } from '$app/stores'
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

	$effect(() => {
		if ($page.url.searchParams.get('error') === 'invalid_client_id') {
			expanded = true
			error = messages.broker.invalidClientId
		}
	})

	function toggle() {
		expanded = !expanded
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

<aside>
	<nav>
		<a href="/strategies" class="nav-link" class:active={$page.url.pathname.startsWith('/strategies')}>
			Strategies
		</a>
		<a href="/runs" class="nav-link" class:active={$page.url.pathname.startsWith('/runs')}>
			Runs
		</a>
	</nav>

	<hr />

	<div class="section">
		<p class="label">Brokers</p>

		<button class="broker" class:connected={$brokerConnected} onclick={toggle}>
			<span class="dot"></span>
			Dhan
		</button>

		{#if expanded}
			<div class="popover">
				{#if $brokerConnected}
					<button class="disconnect-btn" onclick={disconnect}>Disconnect</button>
				{:else}
					<form method="GET" action="/auth/dhan" onsubmit={() => validate() || event.preventDefault()}>
						<input
							type="text"
							name="client_id"
							bind:value={clientId}
							placeholder="10-digit Client ID"
							maxlength="10"
						/>
						{#if error}
							<p class="error">{error}</p>
						{/if}
						<button type="submit" class="connect-btn">Connect</button>
					</form>
				{/if}
			</div>
		{/if}

		<hr />
	</div>
</aside>

<style>
	nav {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-bottom: 8px;
	}

	.nav-link {
		border-radius: 6px;
		color: var(--text-muted);
		display: block;
		font-size: 14px;
		padding: 8px 10px;
		text-decoration: none;
		transition: background 0.15s, color 0.15s;
	}

	.nav-link:hover {
		background: var(--bg);
		color: var(--text);
	}

	.nav-link.active {
		background: var(--bg);
		color: var(--text);
		font-weight: 500;
	}

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
		background: none;
		border: none;
		border-radius: 6px;
		color: var(--red);
		cursor: pointer;
		display: flex;
		font-family: 'Inter', sans-serif;
		font-size: 14px;
		gap: 8px;
		padding: 8px 10px;
		text-align: left;
		transition: background 0.15s;
		width: 100%;
	}

	.broker:hover {
		background: var(--bg);
	}

	.broker.connected {
		color: var(--text);
		cursor: pointer;
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

	.popover {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 8px;
		margin: 4px 0;
		padding: 12px;
	}

	.popover form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.popover input {
		background: var(--bg-surface);
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

	.popover input:focus {
		border-color: var(--accent);
	}

	.popover input::placeholder {
		color: var(--text-faint);
		letter-spacing: normal;
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
		font-family: 'Inter', sans-serif;
		font-size: 13px;
		font-weight: 500;
		padding: 8px;
		transition: background 0.15s, color 0.15s;
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
		font-family: 'Inter', sans-serif;
		font-size: 13px;
		font-weight: 500;
		padding: 8px;
		transition: background 0.15s, color 0.15s;
		width: 100%;
	}

	.disconnect-btn:hover {
		background: var(--red);
		color: #fff;
	}

	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 8px 0;
	}
</style>
