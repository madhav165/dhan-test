# Visual Strategy Builder — Design Spec

**Date:** 2026-05-08

## Goal

Replace the Rust code editor on the strategy new/edit pages with a visual rule builder UI. Users define buy and sell rules through dropdowns and inputs instead of writing Rust code. The visual builder generates Rust code under the hood and feeds it into the existing compilation pipeline unchanged.

## Approach

Code generation: the rule tree is serialized to a Rust function body string before form submission and placed in the existing hidden `<input name="code">`. No backend changes required.

## Rule Model (`src/lib/types/rules.ts`)

```typescript
type Indicator =
  | { name: 'rsi' | 'sma' | 'ema' | 'vwap'; period: number }
  | { name: 'macd'; component: 'macd' | 'signal' | 'histogram'; fast: number; slow: number; signal_period: number }
  | { name: 'bb'; component: 'upper' | 'middle' | 'lower'; period: number }
  | { name: 'volume' }

type Operand =
  | { kind: 'indicator'; indicator: Indicator }
  | { kind: 'number'; value: number }

type Operator = '>' | '<' | '>=' | '<=' | '==' | 'crosses_above' | 'crosses_below'

type Condition = { type: 'condition'; left: Operand; operator: Operator; right: Operand }
type Group     = { type: 'group'; logic: 'AND' | 'OR'; items: (Condition | Group)[] }

export type RuleSet = { buy: Group; sell: Group }
```

## Components

### `RuleBuilder.svelte`
- Top-level component, owns `RuleSet` state
- Two tabs: **Buy** and **Sell**, each rendering the root `Group` for that signal
- On form submit, calls `ruleToRust(ruleset)` and writes the result into a hidden `<input name="code">`
- Replaces `<RustEditor>` in both `new/+page.svelte` and `[id]/edit/+page.svelte`
- For edit page: initialises `RuleSet` from stored rule JSON (new `rule_json` column) if present; otherwise shows empty groups

### `RuleGroup.svelte`
- Props: `group: Group`, `onchange`, `ondelete?` (root groups have no delete)
- Renders AND/OR toggle button
- Lists child items (conditions or nested groups), each with a delete button
- "Add Condition" and "Add Group" buttons at the bottom

### `RuleCondition.svelte`
- Props: `condition: Condition`, `onchange`, `ondelete`
- Left operand picker (indicator name dropdown → parameter inputs)
- Operator dropdown (filtered to relevant operators for the chosen operands)
- Right operand picker: toggle between fixed number input and indicator picker

### `ruleToRust.ts`
- Pure function: `(ruleset: RuleSet) => string`
- Emits declarations for all unique indicator calls at the top
- For MACD/BB, destructures from interleaved output
- Evaluates buy group first (returns 1), then sell group (returns 2), then 0
- `crosses_above(a, b)` → `a_prev < b_prev && a_curr >= b_curr`
- `crosses_below(a, b)` → `a_prev > b_prev && a_curr <= b_curr`
- Volume uses the `volumes` parameter already available in the WASM scaffold

## Database

Add a `rule_json jsonb` column to the `strategies` table (new migration). On save, persist the `RuleSet` JSON alongside the generated Rust code so the edit page can re-hydrate the visual builder. Strategies created via the old code editor will have `rule_json = null` and continue to work (their `code` is still compiled).

## Pages

- `new/+page.svelte`: replace `<RustEditor>` with `<RuleBuilder>`. Start with empty buy/sell groups.
- `[id]/edit/+page.svelte`: replace `<RustEditor>` with `<RuleBuilder initialRules={data.strategy.rule_json} />`. Server must pass `rule_json` in `data.strategy`.
- `[id]/+page.server.ts` (edit action): accept `rule_json` form field and persist it alongside `code`.

## Out of Scope

- Mixing visual builder and code editor in the same strategy
- Previewing generated Rust in the UI
- Validation beyond "at least one condition per signal"
