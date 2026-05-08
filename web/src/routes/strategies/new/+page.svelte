<div class="header">
	<a href="/strategies" class="back">← Strategies</a>
	<h1>New strategy</h1>
</div>

<div class="choices">
	<a href="/strategies/new/rules" class="choice">
		<div class="choice-icon">⚙</div>
		<div class="choice-body">
			<h2>Define rules</h2>
			<p>Build buy and sell conditions using indicators and logic groups.</p>
		</div>
	</a>

	<a href="/strategies/new/rl" class="choice">
		<div class="choice-icon">🧠</div>
		<div class="choice-body">
			<h2>Learn strategy</h2>
			<p>Use reinforcement learning to find an optimal policy from historical data.</p>
		</div>
	</a>
</div>

<style>
	.header { margin-bottom: 24px; }
	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }
	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }

	.choices { display: flex; flex-direction: column; gap: 12px; max-width: 480px; }

	.choice {
		align-items: flex-start;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		display: flex;
		gap: 16px;
		padding: 20px;
		text-decoration: none;
		transition: border-color 0.15s;
	}

	.choice:hover { border-color: var(--accent); }

	.choice-icon { font-size: 1.5rem; line-height: 1; padding-top: 2px; }

	.choice-body h2 { color: var(--text); font-size: 1rem; font-weight: 600; margin: 0 0 4px; }
	.choice-body p { color: var(--text-muted); font-size: 0.85rem; margin: 0; }
</style>
