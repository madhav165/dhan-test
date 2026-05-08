<script lang="ts">
	import { enhance } from '$app/forms'

	let { data, form } = $props()

	let connected = $state(data.telegramConnected)
	let token = $state('')

	const botUrl = `https://t.me/${data.botName}`
	const qrUrl = `https://api.qrserver.com/v1/create-qr-code/?size=160x160&data=${encodeURIComponent(botUrl)}`
</script>

<div class="page">
	<h1>Alerts</h1>

	<section class="card">
		<div class="row">
			<div>
				<h2>Telegram</h2>
				<p class="desc">Receive signal and trade alerts on Telegram.</p>
			</div>

			{#if connected}
				<span class="badge connected">Connected</span>
			{/if}
		</div>

		{#if connected}
			<form method="POST" action="?/disconnect" use:enhance={() => () => { connected = false }}>
				<button type="submit" class="btn danger">Disconnect</button>
			</form>
		{:else}
			<div class="connect-flow">
				<div class="qr-block">
					{#if data.botName}
						<img src={qrUrl} alt="Telegram bot QR code" width="160" height="160" />
						<a href={botUrl} target="_blank" rel="noopener" class="bot-link">Open in Telegram</a>
					{/if}
					<ol class="steps">
						<li>Scan the QR code or click the link above</li>
						<li>Send <code>/start</code> to the bot</li>
						<li>Copy the 6-digit code it sends you</li>
					</ol>
				</div>

				<form
					method="POST"
					action="?/verify"
					class="verify-form"
					use:enhance={() => ({ result }) => {
						if (result.type === 'success') connected = true
					}}
				>
					<input
						type="text"
						name="token"
						bind:value={token}
						placeholder="000000"
						maxlength="6"
						inputmode="numeric"
						autocomplete="one-time-code"
					/>
					{#if form?.error}
						<p class="error">{form.error}</p>
					{/if}
					<button type="submit" class="btn primary" disabled={token.length !== 6}>Connect</button>
				</form>
			</div>
		{/if}
	</section>
</div>

<style>
	.page {
		max-width: 560px;
	}

	h1 {
		font-size: 20px;
		font-weight: 600;
		margin: 0 0 24px;
	}

	.card {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 16px;
	}

	h2 {
		font-size: 15px;
		font-weight: 600;
		margin: 0 0 4px;
	}

	.desc {
		color: var(--text-muted);
		font-size: 13px;
		margin: 0;
	}

	.badge {
		font-size: 11px;
		font-weight: 600;
		padding: 3px 8px;
		border-radius: 20px;
		white-space: nowrap;
	}

	.badge.connected {
		background: color-mix(in srgb, var(--green) 15%, transparent);
		color: var(--green);
	}

	.connect-flow {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.qr-block {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 10px;
	}

	.qr-block img {
		border-radius: 8px;
		border: 1px solid var(--border);
	}

	.bot-link {
		color: var(--accent);
		font-size: 13px;
		text-decoration: none;
	}

	.bot-link:hover {
		text-decoration: underline;
	}

	.steps {
		color: var(--text-muted);
		font-size: 13px;
		margin: 0;
		padding-left: 18px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.steps code {
		background: var(--bg);
		border-radius: 3px;
		padding: 1px 4px;
		font-size: 12px;
	}

	.verify-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.verify-form input {
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 22px;
		letter-spacing: 8px;
		outline: none;
		padding: 10px 14px;
		text-align: center;
		width: 160px;
	}

	.verify-form input:focus {
		border-color: var(--accent);
	}

	.verify-form input::placeholder {
		letter-spacing: 4px;
		font-size: 16px;
		color: var(--text-faint);
	}

	.error {
		color: var(--red);
		font-size: 12px;
		margin: 0;
	}

	.btn {
		border-radius: 6px;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 13px;
		font-weight: 500;
		padding: 8px 16px;
		transition: background 0.15s, color 0.15s;
		border: none;
		width: fit-content;
	}

	.btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.btn.primary {
		background: var(--accent);
		color: #fff;
	}

	.btn.primary:hover:not(:disabled) {
		opacity: 0.85;
	}

	.btn.danger {
		background: none;
		border: 1px solid var(--red);
		color: var(--red);
	}

	.btn.danger:hover {
		background: var(--red);
		color: #fff;
	}
</style>
