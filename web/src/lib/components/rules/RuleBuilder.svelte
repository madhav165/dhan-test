<script lang="ts">
	import { untrack } from 'svelte'
	import type { RuleSet } from '$lib/types/rules'
	import { emptyGroup } from '$lib/types/rules'
	import { ruleToRust } from '$lib/ruleToRust'
	import RuleGroup from './RuleGroup.svelte'

	let { initialRules }: { initialRules?: RuleSet | null } = $props()

	let activeTab: 'buy' | 'sell' = $state('buy')

	let ruleset: RuleSet = $state(untrack(() => initialRules ?? {
		buy: emptyGroup('buy-root'),
		sell: emptyGroup('sell-root'),
	}))

	let generatedCode = $derived(ruleToRust(ruleset))
	let ruleJson = $derived(JSON.stringify(ruleset))
</script>

<div class="builder">
	<div class="tabs">
		<button type="button" class="tab" class:active={activeTab === 'buy'} onclick={() => activeTab = 'buy'}>
			Buy Signal
		</button>
		<button type="button" class="tab" class:active={activeTab === 'sell'} onclick={() => activeTab = 'sell'}>
			Sell Signal
		</button>
	</div>

	<div class="tab-content">
		{#if activeTab === 'buy'}
			<RuleGroup
				group={ruleset.buy}
				onchange={(g) => { ruleset = { ...ruleset, buy: g } }}
			/>
		{:else}
			<RuleGroup
				group={ruleset.sell}
				onchange={(g) => { ruleset = { ...ruleset, sell: g } }}
			/>
		{/if}
	</div>
</div>

<input type="hidden" name="code" value={generatedCode} />
<input type="hidden" name="rule_json" value={ruleJson} />

<style>
	.builder {
		border: 1px solid var(--border);
		border-radius: 6px;
		display: flex;
		flex-direction: column;
	}

	.tabs {
		border-bottom: 1px solid var(--border);
		display: flex;
	}

	.tab {
		background: none;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.85rem;
		padding: 10px 16px;
	}

	.tab.active {
		border-bottom-color: var(--accent);
		color: var(--text);
	}

	.tab-content { padding: 14px; }
</style>
