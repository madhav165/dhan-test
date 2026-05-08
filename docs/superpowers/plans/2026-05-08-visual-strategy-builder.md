# Visual Strategy Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Rust code editor on strategy new/edit pages with a visual rule builder that generates Rust code, requiring no code knowledge from the user.

**Architecture:** The rule tree (buy + sell groups of AND/OR conditions) is held in Svelte state. On form submit, a pure TypeScript function walks the tree and emits a Rust function body string into the existing hidden `<input name="code">`. A `rule_json` column persists the tree so the edit page can re-hydrate the builder. No backend compilation changes.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, PostgreSQL (migration)

---

### Task 1: DB migration — add `rule_json` to strategies

**Files:**
- Create: `go/cmd/server/migrations/014_strategy_rule_json.up.sql`
- Create: `go/cmd/server/migrations/014_strategy_rule_json.down.sql`

- [ ] **Step 1: Write up migration**

`go/cmd/server/migrations/014_strategy_rule_json.up.sql`:
```sql
alter table strategies add column rule_json jsonb;
```

- [ ] **Step 2: Write down migration**

`go/cmd/server/migrations/014_strategy_rule_json.down.sql`:
```sql
alter table strategies drop column rule_json;
```

- [ ] **Step 3: Apply migration**

```bash
# run however migrations are applied in this project
# check Makefile for the migrate target
make migrate
```

Expected: no error, `\d strategies` shows `rule_json jsonb` column.

- [ ] **Step 4: Commit**

```bash
git add go/cmd/server/migrations/014_strategy_rule_json.up.sql go/cmd/server/migrations/014_strategy_rule_json.down.sql
git commit -m "Add rule_json column to strategies"
```

---

### Task 2: Rule model types

**Files:**
- Create: `web/src/lib/types/rules.ts`

- [ ] **Step 1: Write the types**

`web/src/lib/types/rules.ts`:
```typescript
export type IndicatorSingle = {
	name: 'rsi' | 'sma' | 'ema' | 'vwap'
	period: number
}

export type IndicatorMacd = {
	name: 'macd'
	component: 'macd' | 'signal' | 'histogram'
	fast: number
	slow: number
	signal_period: number
}

export type IndicatorBb = {
	name: 'bb'
	component: 'upper' | 'middle' | 'lower'
	period: number
}

export type IndicatorVolume = { name: 'volume' }

export type Indicator = IndicatorSingle | IndicatorMacd | IndicatorBb | IndicatorVolume

export type Operand =
	| { kind: 'indicator'; indicator: Indicator }
	| { kind: 'number'; value: number }

export type Operator = '>' | '<' | '>=' | '<=' | '==' | 'crosses_above' | 'crosses_below'

export type Condition = {
	type: 'condition'
	id: string
	left: Operand
	operator: Operator
	right: Operand
}

export type Group = {
	type: 'group'
	id: string
	logic: 'AND' | 'OR'
	items: (Condition | Group)[]
}

export type RuleSet = { buy: Group; sell: Group }

export function emptyGroup(id: string): Group {
	return { type: 'group', id, logic: 'AND', items: [] }
}

export function emptyCondition(id: string): Condition {
	return {
		type: 'condition',
		id,
		left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
		operator: '>',
		right: { kind: 'number', value: 30 },
	}
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/types/rules.ts
git commit -m "Add rule model types"
```

---

### Task 3: Rust code generator

**Files:**
- Create: `web/src/lib/ruleToRust.ts`
- Create: `web/src/lib/ruleToRust.test.ts`

- [ ] **Step 1: Write failing tests**

`web/src/lib/ruleToRust.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { ruleToRust } from './ruleToRust'
import type { RuleSet } from './types/rules'

function makeRuleSet(overrides: Partial<RuleSet> = {}): RuleSet {
	return {
		buy: {
			type: 'group', id: 'b', logic: 'AND',
			items: [{
				type: 'condition', id: 'c1',
				left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
				operator: '<',
				right: { kind: 'number', value: 30 },
			}],
		},
		sell: {
			type: 'group', id: 's', logic: 'AND',
			items: [{
				type: 'condition', id: 'c2',
				left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
				operator: '>',
				right: { kind: 'number', value: 70 },
			}],
		},
		...overrides,
	}
}

describe('ruleToRust', () => {
	it('declares rsi once even when used in both rules', () => {
		const code = ruleToRust(makeRuleSet())
		const matches = code.match(/let rsi_14/g)
		expect(matches?.length).toBe(1)
	})

	it('returns 1 for buy condition', () => {
		const code = ruleToRust(makeRuleSet())
		expect(code).toContain('return 1')
	})

	it('returns 2 for sell condition', () => {
		const code = ruleToRust(makeRuleSet())
		expect(code).toContain('return 2')
	})

	it('returns 0 at end', () => {
		const code = ruleToRust(makeRuleSet())
		expect(code.trimEnd()).toMatch(/0\s*$/)
	})

	it('handles crosses_above with prev/curr', () => {
		const rs: RuleSet = {
			buy: {
				type: 'group', id: 'b', logic: 'AND',
				items: [{
					type: 'condition', id: 'c1',
					left: { kind: 'indicator', indicator: { name: 'ema', period: 9 } },
					operator: 'crosses_above',
					right: { kind: 'indicator', indicator: { name: 'ema', period: 21 } },
				}],
			},
			sell: { type: 'group', id: 's', logic: 'AND', items: [] },
		}
		const code = ruleToRust(rs)
		expect(code).toContain('ema_9_prev < ema_21_prev')
		expect(code).toContain('ema_9_curr >= ema_21_curr')
	})

	it('handles volume operand', () => {
		const rs: RuleSet = {
			buy: {
				type: 'group', id: 'b', logic: 'AND',
				items: [{
					type: 'condition', id: 'c1',
					left: { kind: 'indicator', indicator: { name: 'volume' } },
					operator: '>',
					right: { kind: 'number', value: 1000000 },
				}],
			},
			sell: { type: 'group', id: 's', logic: 'AND', items: [] },
		}
		const code = ruleToRust(rs)
		expect(code).toContain('volumes[i]')
	})

	it('handles OR logic in group', () => {
		const rs: RuleSet = {
			buy: {
				type: 'group', id: 'b', logic: 'OR',
				items: [
					{
						type: 'condition', id: 'c1',
						left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
						operator: '<',
						right: { kind: 'number', value: 30 },
					},
					{
						type: 'condition', id: 'c2',
						left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
						operator: '<',
						right: { kind: 'number', value: 25 },
					},
				],
			},
			sell: { type: 'group', id: 's', logic: 'AND', items: [] },
		}
		const code = ruleToRust(rs)
		expect(code).toContain('||')
	})

	it('handles macd component', () => {
		const rs: RuleSet = {
			buy: {
				type: 'group', id: 'b', logic: 'AND',
				items: [{
					type: 'condition', id: 'c1',
					left: {
						kind: 'indicator',
						indicator: { name: 'macd', component: 'histogram', fast: 12, slow: 26, signal_period: 9 },
					},
					operator: '>',
					right: { kind: 'number', value: 0 },
				}],
			},
			sell: { type: 'group', id: 's', logic: 'AND', items: [] },
		}
		const code = ruleToRust(rs)
		expect(code).toContain('macd_12_26_9_histogram')
	})

	it('empty groups produce no early return', () => {
		const rs: RuleSet = {
			buy: { type: 'group', id: 'b', logic: 'AND', items: [] },
			sell: { type: 'group', id: 's', logic: 'AND', items: [] },
		}
		const code = ruleToRust(rs)
		expect(code).not.toContain('return 1')
		expect(code).not.toContain('return 2')
	})
})
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd web && npx vitest run src/lib/ruleToRust.test.ts
```

Expected: all tests fail with module not found.

- [ ] **Step 3: Implement the generator**

`web/src/lib/ruleToRust.ts`:
```typescript
import type { Condition, Group, Indicator, Operand, Operator, RuleSet } from './types/rules'

// Returns a stable string key for an indicator (used for dedup + variable naming)
function indicatorKey(ind: Indicator): string {
	if (ind.name === 'volume') return 'volume'
	if (ind.name === 'macd') return `macd_${ind.fast}_${ind.slow}_${ind.signal_period}`
	if (ind.name === 'bb') return `bb_${ind.period}`
	return `${ind.name}_${ind.period}`
}

// Returns the Rust expression for an operand at index i
function operandExpr(op: Operand, suffix: 'curr' | 'prev' | null): string {
	if (op.kind === 'number') return `${op.value}.0`
	const ind = op.indicator
	if (ind.name === 'volume') {
		const idx = suffix === 'prev' ? 'i - 1' : 'i'
		return `volumes[${idx}]`
	}
	const key = indicatorKey(ind)
	const idx = suffix === 'prev' ? 'i - 1' : 'i'
	if (ind.name === 'macd') return `${key}_${ind.component}[${idx}]`
	if (ind.name === 'bb') return `${key}_${ind.component}[${idx}]`
	return `${key}[${idx}]`
}

function varName(op: Operand, suffix: string): string {
	if (op.kind === 'number') return `${op.value}`
	const ind = op.indicator
	if (ind.name === 'volume') return `volume_${suffix}`
	const key = indicatorKey(ind)
	if (ind.name === 'macd') return `${key}_${ind.component}_${suffix}`
	if (ind.name === 'bb') return `${key}_${ind.component}_${suffix}`
	return `${key}_${suffix}`
}

// Collect all unique indicators from a group recursively
function collectIndicators(group: Group): Set<string> {
	const keys = new Set<string>()
	function walk(g: Group) {
		for (const item of g.items) {
			if (item.type === 'condition') {
				if (item.left.kind === 'indicator') keys.add(indicatorKey(item.left.indicator))
				if (item.right.kind === 'indicator') keys.add(indicatorKey(item.right.indicator))
			} else {
				walk(item)
			}
		}
	}
	walk(group)
	return keys
}

function collectAllIndicators(rs: RuleSet): Map<string, Indicator> {
	const map = new Map<string, Indicator>()
	function walk(g: Group) {
		for (const item of g.items) {
			if (item.type === 'condition') {
				for (const op of [item.left, item.right]) {
					if (op.kind === 'indicator') map.set(indicatorKey(op.indicator), op.indicator)
				}
			} else walk(item)
		}
	}
	walk(rs.buy)
	walk(rs.sell)
	return map
}

function needsPrevCurr(group: Group): boolean {
	for (const item of group.items) {
		if (item.type === 'condition') {
			if (item.operator === 'crosses_above' || item.operator === 'crosses_below') return true
		} else if (needsPrevCurr(item)) return true
	}
	return false
}

function emitDeclarations(indicators: Map<string, Indicator>, needsCross: boolean): string {
	const lines: string[] = []
	for (const [key, ind] of indicators) {
		if (ind.name === 'volume') continue // volumes already available
		if (ind.name === 'rsi' || ind.name === 'sma' || ind.name === 'ema' || ind.name === 'vwap') {
			lines.push(`    let ${key} = ${ind.name}(prices, ${ind.period});`)
		} else if (ind.name === 'macd') {
			lines.push(`    let (${key}_macd, ${key}_signal, ${key}_histogram) = macd(prices, ${ind.fast}, ${ind.slow}, ${ind.signal_period});`)
		} else if (ind.name === 'bb') {
			lines.push(`    let (${key}_upper, ${key}_middle, ${key}_lower) = bb(prices, ${ind.period});`)
		}
	}
	return lines.join('\n')
}

function emitConditionExpr(cond: Condition): string {
	const op = cond.operator
	if (op === 'crosses_above') {
		const lp = varName(cond.left, 'prev'), lc = varName(cond.left, 'curr')
		const rp = varName(cond.right, 'prev'), rc = varName(cond.right, 'curr')
		const lpe = operandExpr(cond.left, 'prev'), lce = operandExpr(cond.left, 'curr')
		const rpe = operandExpr(cond.right, 'prev'), rce = operandExpr(cond.right, 'curr')
		return `({ let ${lp} = ${lpe}; let ${lc} = ${lce}; let ${rp} = ${rpe}; let ${rc} = ${rce}; ${lp} < ${rp} && ${lc} >= ${rc} })`
	}
	if (op === 'crosses_below') {
		const lp = varName(cond.left, 'prev'), lc = varName(cond.left, 'curr')
		const rp = varName(cond.right, 'prev'), rc = varName(cond.right, 'curr')
		const lpe = operandExpr(cond.left, 'prev'), lce = operandExpr(cond.left, 'curr')
		const rpe = operandExpr(cond.right, 'prev'), rce = operandExpr(cond.right, 'curr')
		return `({ let ${lp} = ${lpe}; let ${lc} = ${lce}; let ${rp} = ${rpe}; let ${rc} = ${rce}; ${lp} > ${rp} && ${lc} <= ${rc} })`
	}
	const l = operandExpr(cond.left, 'curr')
	const r = operandExpr(cond.right, 'curr')
	return `(${l} ${op} ${r})`
}

function emitGroupExpr(group: Group): string | null {
	if (group.items.length === 0) return null
	const parts: string[] = []
	for (const item of group.items) {
		if (item.type === 'condition') {
			parts.push(emitConditionExpr(item))
		} else {
			const sub = emitGroupExpr(item)
			if (sub) parts.push(`(${sub})`)
		}
	}
	if (parts.length === 0) return null
	return parts.join(group.logic === 'AND' ? ' && ' : ' || ')
}

export function ruleToRust(rs: RuleSet): string {
	const indicators = collectAllIndicators(rs)
	const decls = emitDeclarations(indicators, false)

	const buyExpr = emitGroupExpr(rs.buy)
	const sellExpr = emitGroupExpr(rs.sell)

	const lines: string[] = []
	if (decls) lines.push(decls)
	if (buyExpr) lines.push(`    if ${buyExpr} { return 1; }`)
	if (sellExpr) lines.push(`    if ${sellExpr} { return 2; }`)
	lines.push(`    0`)

	return lines.join('\n')
}
```

- [ ] **Step 4: Run tests**

```bash
cd web && npx vitest run src/lib/ruleToRust.test.ts
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/ruleToRust.ts web/src/lib/ruleToRust.test.ts
git commit -m "Add Rust code generator for visual rule builder"
```

---

### Task 4: RuleCondition component

**Files:**
- Create: `web/src/lib/components/rules/RuleCondition.svelte`

- [ ] **Step 1: Create the component**

`web/src/lib/components/rules/RuleCondition.svelte`:
```svelte
<script lang="ts">
	import type { Condition, Indicator, Operand, Operator } from '$lib/types/rules'

	let { condition, onchange, ondelete }: {
		condition: Condition
		onchange: (c: Condition) => void
		ondelete: () => void
	} = $props()

	const indicatorNames = ['rsi', 'sma', 'ema', 'vwap', 'macd', 'bb', 'volume'] as const
	const operators: Operator[] = ['>', '<', '>=', '<=', '==', 'crosses_above', 'crosses_below']
	const macdComponents = ['macd', 'signal', 'histogram'] as const
	const bbComponents = ['upper', 'middle', 'lower'] as const

	function defaultIndicator(name: string): Indicator {
		if (name === 'macd') return { name: 'macd', component: 'macd', fast: 12, slow: 26, signal_period: 9 }
		if (name === 'bb') return { name: 'bb', component: 'upper', period: 20 }
		if (name === 'volume') return { name: 'volume' }
		return { name: name as any, period: 14 }
	}

	function setLeftIndicatorName(name: string) {
		onchange({ ...condition, left: { kind: 'indicator', indicator: defaultIndicator(name) } })
	}

	function setRightKind(kind: 'indicator' | 'number') {
		if (kind === 'number') {
			onchange({ ...condition, right: { kind: 'number', value: 0 } })
		} else {
			onchange({ ...condition, right: { kind: 'indicator', indicator: defaultIndicator('rsi') } })
		}
	}

	function setRightIndicatorName(name: string) {
		onchange({ ...condition, right: { kind: 'indicator', indicator: defaultIndicator(name) } })
	}

	function patchLeftIndicator(patch: Partial<Indicator>) {
		if (condition.left.kind !== 'indicator') return
		onchange({ ...condition, left: { kind: 'indicator', indicator: { ...condition.left.indicator, ...patch } as Indicator } })
	}

	function patchRightIndicator(patch: Partial<Indicator>) {
		if (condition.right.kind !== 'indicator') return
		onchange({ ...condition, right: { kind: 'indicator', indicator: { ...condition.right.indicator, ...patch } as Indicator } })
	}
</script>

<div class="condition">
	<!-- Left operand (always indicator) -->
	<select value={condition.left.kind === 'indicator' ? condition.left.indicator.name : 'rsi'}
		onchange={(e) => setLeftIndicatorName((e.target as HTMLSelectElement).value)}>
		{#each indicatorNames as n}
			<option value={n}>{n.toUpperCase()}</option>
		{/each}
	</select>

	{#if condition.left.kind === 'indicator' && condition.left.indicator.name !== 'volume'}
		{#if condition.left.indicator.name === 'macd'}
			<select value={condition.left.indicator.component}
				onchange={(e) => patchLeftIndicator({ component: (e.target as HTMLSelectElement).value as any })}>
				{#each macdComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.left.indicator.fast} min="1"
				oninput={(e) => patchLeftIndicator({ fast: +(e.target as HTMLInputElement).value })} placeholder="fast" class="num-sm" />
			<input type="number" value={condition.left.indicator.slow} min="1"
				oninput={(e) => patchLeftIndicator({ slow: +(e.target as HTMLInputElement).value })} placeholder="slow" class="num-sm" />
			<input type="number" value={condition.left.indicator.signal_period} min="1"
				oninput={(e) => patchLeftIndicator({ signal_period: +(e.target as HTMLInputElement).value })} placeholder="sig" class="num-sm" />
		{:else if condition.left.indicator.name === 'bb'}
			<select value={condition.left.indicator.component}
				onchange={(e) => patchLeftIndicator({ component: (e.target as HTMLSelectElement).value as any })}>
				{#each bbComponents as c}<option value={c}>{c}</option>{/each}
			</select>
			<input type="number" value={condition.left.indicator.period} min="1"
				oninput={(e) => patchLeftIndicator({ period: +(e.target as HTMLInputElement).value })} class="num-sm" />
		{:else}
			<input type="number" value={condition.left.indicator.period} min="1"
				oninput={(e) => patchLeftIndicator({ period: +(e.target as HTMLInputElement).value })} class="num-sm" />
		{/if}
	{/if}

	<!-- Operator -->
	<select value={condition.operator}
		onchange={(e) => onchange({ ...condition, operator: (e.target as HTMLSelectElement).value as Operator })}>
		{#each operators as op}<option value={op}>{op}</option>{/each}
	</select>

	<!-- Right operand -->
	<select value={condition.right.kind}
		onchange={(e) => setRightKind((e.target as HTMLSelectElement).value as any)}>
		<option value="number">Number</option>
		<option value="indicator">Indicator</option>
	</select>

	{#if condition.right.kind === 'number'}
		<input type="number" value={condition.right.value}
			oninput={(e) => onchange({ ...condition, right: { kind: 'number', value: +(e.target as HTMLInputElement).value } })}
			class="num-md" />
	{:else if condition.right.kind === 'indicator'}
		<select value={condition.right.indicator.name}
			onchange={(e) => setRightIndicatorName((e.target as HTMLSelectElement).value)}>
			{#each indicatorNames as n}
				<option value={n}>{n.toUpperCase()}</option>
			{/each}
		</select>

		{#if condition.right.indicator.name !== 'volume'}
			{#if condition.right.indicator.name === 'macd'}
				<select value={condition.right.indicator.component}
					onchange={(e) => patchRightIndicator({ component: (e.target as HTMLSelectElement).value as any })}>
					{#each macdComponents as c}<option value={c}>{c}</option>{/each}
				</select>
				<input type="number" value={condition.right.indicator.fast} min="1"
					oninput={(e) => patchRightIndicator({ fast: +(e.target as HTMLInputElement).value })} class="num-sm" />
				<input type="number" value={condition.right.indicator.slow} min="1"
					oninput={(e) => patchRightIndicator({ slow: +(e.target as HTMLInputElement).value })} class="num-sm" />
				<input type="number" value={condition.right.indicator.signal_period} min="1"
					oninput={(e) => patchRightIndicator({ signal_period: +(e.target as HTMLInputElement).value })} class="num-sm" />
			{:else if condition.right.indicator.name === 'bb'}
				<select value={condition.right.indicator.component}
					onchange={(e) => patchRightIndicator({ component: (e.target as HTMLSelectElement).value as any })}>
					{#each bbComponents as c}<option value={c}>{c}</option>{/each}
				</select>
				<input type="number" value={condition.right.indicator.period} min="1"
					oninput={(e) => patchRightIndicator({ period: +(e.target as HTMLInputElement).value })} class="num-sm" />
			{:else}
				<input type="number" value={condition.right.indicator.period} min="1"
					oninput={(e) => patchRightIndicator({ period: +(e.target as HTMLInputElement).value })} class="num-sm" />
			{/if}
		{/if}
	{/if}

	<button type="button" class="del" onclick={ondelete}>✕</button>
</div>

<style>
	.condition {
		align-items: center;
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	select, input {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text);
		font-size: 0.8rem;
		padding: 4px 6px;
	}

	.num-sm { width: 56px; }
	.num-md { width: 80px; }

	.del {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.75rem;
		padding: 2px 4px;
	}

	.del:hover { color: var(--red-muted); }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/components/rules/RuleCondition.svelte
git commit -m "Add RuleCondition component"
```

---

### Task 5: RuleGroup component

**Files:**
- Create: `web/src/lib/components/rules/RuleGroup.svelte`

- [ ] **Step 1: Create the component**

`web/src/lib/components/rules/RuleGroup.svelte`:
```svelte
<script lang="ts">
	import type { Condition, Group } from '$lib/types/rules'
	import { emptyCondition, emptyGroup } from '$lib/types/rules'
	import RuleCondition from './RuleCondition.svelte'

	let { group, onchange, ondelete }: {
		group: Group
		onchange: (g: Group) => void
		ondelete?: () => void
	} = $props()

	let nextId = $state(0)
	function uid() { return `${Date.now()}-${nextId++}` }

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

	<div class="items">
		{#each group.items as item, i}
			{#if item.type === 'condition'}
				<RuleCondition
					condition={item}
					onchange={(c) => updateItem(i, c)}
					ondelete={() => deleteItem(i)}
				/>
			{:else}
				<!-- Recursive group -->
				<svelte:self
					group={item}
					onchange={(g) => updateItem(i, g)}
					ondelete={() => deleteItem(i)}
				/>
			{/if}
		{/each}
	</div>

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
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/components/rules/RuleGroup.svelte
git commit -m "Add RuleGroup component"
```

---

### Task 6: RuleBuilder top-level component

**Files:**
- Create: `web/src/lib/components/rules/RuleBuilder.svelte`

- [ ] **Step 1: Create the component**

`web/src/lib/components/rules/RuleBuilder.svelte`:
```svelte
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

<!-- Hidden inputs consumed by the parent form -->
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
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/components/rules/RuleBuilder.svelte
git commit -m "Add RuleBuilder top-level component"
```

---

### Task 7: Wire up new strategy page

**Files:**
- Modify: `web/src/routes/strategies/new/+page.svelte`
- Modify: `web/src/routes/strategies/new/+page.server.ts`

- [ ] **Step 1: Update the page**

Replace all content of `web/src/routes/strategies/new/+page.svelte` with:

```svelte
<script lang="ts">
	import { enhance } from '$app/forms'
	import RuleBuilder from '$lib/components/rules/RuleBuilder.svelte'

	let { form } = $props()
</script>

<div class="header">
	<a href="/strategies" class="back">← Strategies</a>
	<h1>New strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	{#if form?.error}
		<p class="error">{form.error}</p>
	{/if}

	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" placeholder="e.g. RSI mean reversion" required />
	</div>

	<div class="field">
		<label>Signal rules</label>
		<RuleBuilder />
	</div>

	<div class="footer">
		<a href="/strategies" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Create</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }

	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }

	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }

	.form { display: flex; flex-direction: column; gap: 20px; max-width: 720px; }

	.field { display: flex; flex-direction: column; gap: 6px; }

	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }

	input[type='text'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}

	input[type='text']:focus { border-color: var(--accent); }

	.footer { display: flex; gap: 12px; justify-content: flex-end; padding-top: 8px; }

	.btn-primary {
		background: var(--accent);
		border: none;
		border-radius: 6px;
		color: #000;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 20px;
	}

	.btn-primary:hover { background: var(--accent-hover); }

	.btn-secondary {
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		font-size: 0.875rem;
		padding: 8px 20px;
		text-decoration: none;
	}

	.btn-secondary:hover { color: var(--text); }

	.error {
		background: var(--red-bg);
		border-radius: 6px;
		color: var(--red-muted);
		font-size: 0.85rem;
		padding: 10px 14px;
	}
</style>
```

- [ ] **Step 2: Update server action to persist rule_json**

In `web/src/routes/strategies/new/+page.server.ts`, update the default action:

```typescript
import { db } from '$lib/server/db'
import { redirect, fail } from '@sveltejs/kit'
import type { Actions } from './$types'

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const code = form.get('code')?.toString().trim()
		const rule_json = form.get('rule_json')?.toString() ?? null

		if (!name) return fail(400, { error: 'Name is required' })
		if (!code) return fail(400, { error: 'Signal logic is required' })

		const stratResult = await db.query(
			`insert into strategies (user_id, name, rule_json) values ($1, $2, $3) returning id`,
			[locals.user!.id, name, rule_json ? JSON.parse(rule_json) : null]
		)
		const strategyId = stratResult.rows[0].id

		await db.query(
			`update strategies set source_key = $1 where id = $2`,
			[code, strategyId]
		)

		await db.query(
			`insert into build_jobs (strategy_id) values ($1)`,
			[strategyId]
		)

		redirect(302, `/strategies/${strategyId}`)
	},
}
```

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/strategies/new/+page.svelte web/src/routes/strategies/new/+page.server.ts
git commit -m "Wire visual rule builder into new strategy page"
```

---

### Task 8: Wire up edit strategy page

**Files:**
- Modify: `web/src/routes/strategies/[id]/edit/+page.svelte`
- Modify: `web/src/routes/strategies/[id]/edit/+page.server.ts`

- [ ] **Step 1: Update load to fetch rule_json**

In `web/src/routes/strategies/[id]/edit/+page.server.ts`, update the load query:

```typescript
import { GO_URL } from '$env/static/private'
import { db } from '$lib/server/db'
import { error, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select id, name, rule_json from strategies where id = $1 and user_id = $2`,
		[params.id, locals.user!.id]
	)
	if (result.rows.length === 0) error(404, 'Strategy not found')
	return { strategy: result.rows[0] }
}

export const actions: Actions = {
	default: async ({ request, locals, params }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const code = form.get('code')?.toString().trim()
		const rule_json = form.get('rule_json')?.toString() ?? null

		if (!name || !code) error(400, 'Name and code are required')

		await db.query(
			`update strategies set name = $1, source_key = $2, wasm_key = null, rule_json = $3 where id = $4 and user_id = $5`,
			[name, code, rule_json ? JSON.parse(rule_json) : null, params.id, locals.user!.id]
		)
		await db.query(`insert into build_jobs (strategy_id) values ($1)`, [params.id])

		redirect(302, `/strategies/${params.id}`)
	},
}
```

- [ ] **Step 2: Update edit page**

Replace all content of `web/src/routes/strategies/[id]/edit/+page.svelte` with:

```svelte
<script lang="ts">
	import { untrack } from 'svelte'
	import { enhance } from '$app/forms'
	import RuleBuilder from '$lib/components/rules/RuleBuilder.svelte'
	import type { RuleSet } from '$lib/types/rules'

	let { data } = $props()

	let name = $state(untrack(() => data.strategy.name))
	let initialRules: RuleSet | null = untrack(() => data.strategy.rule_json ?? null)
</script>

<div class="header">
	<a href="/strategies/{data.strategy.id}" class="back">← {data.strategy.name}</a>
	<h1>Edit strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" bind:value={name} required />
	</div>

	<div class="field">
		<label>Signal rules</label>
		<RuleBuilder {initialRules} />
	</div>

	<div class="footer">
		<a href="/strategies/{data.strategy.id}" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Save & rebuild</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }
	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }
	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }
	.form { display: flex; flex-direction: column; gap: 20px; max-width: 720px; }
	.field { display: flex; flex-direction: column; gap: 6px; }
	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }
	input[type='text'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}
	input[type='text']:focus { border-color: var(--accent); }
	.footer { display: flex; gap: 12px; justify-content: flex-end; padding-top: 8px; }
	.btn-primary {
		background: var(--accent);
		border: none;
		border-radius: 6px;
		color: #000;
		cursor: pointer;
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		font-weight: 500;
		padding: 8px 20px;
	}
	.btn-primary:hover { background: var(--accent-hover); }
	.btn-secondary {
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-muted);
		font-size: 0.875rem;
		padding: 8px 20px;
		text-decoration: none;
	}
	.btn-secondary:hover { color: var(--text); }
</style>
```

- [ ] **Step 3: Commit**

```bash
git add "web/src/routes/strategies/[id]/edit/+page.svelte" "web/src/routes/strategies/[id]/edit/+page.server.ts"
git commit -m "Wire visual rule builder into edit strategy page"
```

---

### Task 9: Manual smoke test

- [ ] **Step 1: Start dev server**

```bash
cd web && npm run dev
```

- [ ] **Step 2: Test new strategy flow**

Navigate to `/strategies/new`. Confirm:
- Buy/Sell tabs visible
- Can add conditions with indicator + operator + number
- Can add conditions with indicator + operator + indicator
- Can toggle AND/OR on a group
- Can add nested groups
- Can delete conditions and groups
- Submitting creates a strategy and redirects to the strategy page
- Build job kicks off (check DB: `select * from build_jobs order by created_at desc limit 1`)

- [ ] **Step 3: Test edit strategy flow**

Navigate to an existing strategy's edit page. Confirm:
- If `rule_json` is populated, the builder re-hydrates with the saved rules
- Saving re-queues a build job

- [ ] **Step 4: Verify generated Rust**

Check `source_key` in the DB for a strategy with a simple RSI < 30 buy rule:

```sql
select source_key from strategies order by created_at desc limit 1;
```

Expected output should look like:
```
    let rsi_14 = rsi(prices, 14);
    if (rsi_14[i] < 30.0) { return 1; }
    0
```
