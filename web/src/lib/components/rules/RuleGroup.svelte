<script lang="ts">
	import type { Condition, Group } from '$lib/types/rules'
	import { emptyCondition, emptyGroup } from '$lib/types/rules'
	import RuleCondition from './RuleCondition.svelte'

	let { group, onchange, ondelete }: {
		group: Group
		onchange: (g: Group) => void
		ondelete?: () => void
	} = $props()

	let counter = $state(0)
	function uid() { return `${Date.now()}-${counter++}` }

	function toggleLogic() {
		onchange({ ...group, logic: group.logic === 'AND' ? 'OR' : 'AND' })
	}

	function addCondition() {
		onchange({ ...group, items: [...group.items, emptyCondition(uid())] })
	}

	function addGroup() {
		onchange({ ...group, items: [...group.items, emptyGroup(uid())] })
	}

	function updateItem(index: number, item: Condition | Group) {
		const items = [...group.items]
		items[index] = item
		onchange({ ...group, items })
	}

	function deleteItem(index: number) {
		onchange({ ...group, items: group.items.filter((_, i) => i !== index) })
	}
</script>

<div class="group">
	<div class="group-header">
		<button type="button" class="logic-toggle" onclick={toggleLogic}>{group.logic}</button>
		<span class="label">{group.logic === 'AND' ? 'All must be true' : 'Any must be true'}</span>
		{#if ondelete}
			<button type="button" class="del" onclick={ondelete}>✕ Remove group</button>
		{/if}
	</div>

	{#if group.items.length > 0}
		<div class="items">
			{#each group.items as item, i}
				{#if item.type === 'condition'}
					<RuleCondition
						condition={item}
						onchange={(c) => updateItem(i, c)}
						ondelete={() => deleteItem(i)}
					/>
				{:else}
					<svelte:self
						group={item}
						onchange={(g) => updateItem(i, g)}
						ondelete={() => deleteItem(i)}
					/>
				{/if}
			{/each}
		</div>
	{/if}

	<div class="actions">
		<button type="button" class="btn-add" onclick={addCondition}>+ Add Condition</button>
		<button type="button" class="btn-add" onclick={addGroup}>+ Add Group</button>
	</div>
</div>

<style>
	.group {
		border: 1px solid var(--border);
		border-radius: 6px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
	}

	.group-header {
		align-items: center;
		display: flex;
		gap: 10px;
	}

	.logic-toggle {
		background: var(--accent);
		border: none;
		border-radius: 4px;
		color: #000;
		cursor: pointer;
		font-size: 0.75rem;
		font-weight: 600;
		padding: 3px 10px;
	}

	.label {
		color: var(--text-muted);
		font-size: 0.8rem;
	}

	.del {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.75rem;
		margin-left: auto;
	}

	.del:hover { color: var(--red-muted); }

	.items {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-left: 12px;
	}

	.actions { display: flex; gap: 8px; }

	.btn-add {
		background: none;
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.8rem;
		padding: 4px 10px;
	}

	.btn-add:hover { border-color: var(--accent); color: var(--accent); }
</style>
