# RL Strategy Training Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the training half of the RL strategy feature — DB schema, RL config UI (reward, constraints, indicator selection, training date range, allow_short toggle), and the PPO training loop in the builder that produces trained weights and an interpretable summary.

**Architecture:** User configures an RL job through a new `/strategies/new/rl` page; the form creates a strategy row with `strategy_type='rl'` and an `rl_jobs` row. The builder polls `rl_jobs`, loads candles, computes indicator features, runs a PPO training loop in native Rust, serialises weights to MinIO, runs policy distillation (decision tree) and feature importance (input perturbation), and writes an `rl_summary` JSON back to the strategy row. Inference WASM is a separate plan.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, PostgreSQL, Rust (native builder binary), `ndarray` + `rand` crates for PPO, existing MinIO/S3 upload helpers.

---

## File Structure

**New files:**
- `go/cmd/server/migrations/015_rl_strategy.up.sql` — adds `strategy_type`, `rl_config`, `rl_summary` columns + `rl_jobs` table
- `go/cmd/server/migrations/015_rl_strategy.down.sql`
- `web/src/lib/types/rl.ts` — `RLConfig`, `RLSummary` TypeScript types
- `web/src/routes/strategies/new/+page.svelte` — updated to show choice (rules vs learn)
- `web/src/routes/strategies/new/rl/+page.svelte` — RL config form
- `web/src/routes/strategies/new/rl/+page.server.ts` — creates strategy + rl_job
- `web/src/routes/strategies/[id]/+page.svelte` — updated to show RL summary if strategy_type=rl
- `web/src/routes/strategies/[id]/+page.server.ts` — updated to load rl_config, rl_summary
- `builder/src/rl/mod.rs` — PPO trainer (state, network, training loop)
- `builder/src/rl/features.rs` — feature computation (indicators + OHLCV window)
- `builder/src/rl/distill.rs` — decision tree distillation + feature importance
- `builder/src/main.rs` — add `rl_jobs` polling loop

**Modified files:**
- `builder/Cargo.toml` — add `ndarray`, `rand`, `rand_distr`

---

### Task 1: DB migrations

**Files:**
- Create: `go/cmd/server/migrations/015_rl_strategy.up.sql`
- Create: `go/cmd/server/migrations/015_rl_strategy.down.sql`

- [ ] **Step 1: Write up migration**

`go/cmd/server/migrations/015_rl_strategy.up.sql`:
```sql
alter table strategies add column strategy_type text not null default 'manual';
alter table strategies add column rl_config jsonb;
alter table strategies add column rl_summary jsonb;

create table rl_jobs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  status      text not null default 'pending',
  error       text,
  created_at  timestamptz default now(),
  updated_at  timestamptz default now()
);
```

- [ ] **Step 2: Write down migration**

`go/cmd/server/migrations/015_rl_strategy.down.sql`:
```sql
drop table if exists rl_jobs;
alter table strategies drop column if exists rl_summary;
alter table strategies drop column if exists rl_config;
alter table strategies drop column if exists strategy_type;
```

- [ ] **Step 3: Apply migration**

Check `Makefile` for the migrate target and run it. Verify with:
```bash
make migrate 2>/dev/null || echo "check Makefile for migrate target"
```

- [ ] **Step 4: Commit**

```bash
git add go/cmd/server/migrations/015_rl_strategy.up.sql go/cmd/server/migrations/015_rl_strategy.down.sql
git commit -m "Add rl_jobs table and rl columns to strategies"
```

---

### Task 2: TypeScript types for RL config

**Files:**
- Create: `web/src/lib/types/rl.ts`

- [ ] **Step 1: Write the types**

`web/src/lib/types/rl.ts`:
```typescript
import type { Indicator } from './rules'

export type RLReward = 'pnl' | 'sharpe' | 'min_drawdown'

export type RLConstraint =
	| { type: 'max_holding_days'; value: number }
	| { type: 'max_trades_per_month'; value: number }

export type RLConfig = {
	reward: RLReward
	constraints: RLConstraint[]
	indicators: Indicator[]
	lookback_candles: number
	allow_short: boolean
	train_from: string  // YYYY-MM-DD
	train_to: string    // YYYY-MM-DD
}

export type FeatureImportance = {
	name: string
	importance: number  // 0–1, higher = more influential
}

export type RLSummary = {
	feature_importance: FeatureImportance[]
	approximate_rules: string  // human-readable decision tree text
	training_episodes: number
	final_reward: number
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/types/rl.ts
git commit -m "Add RLConfig and RLSummary TypeScript types"
```

---

### Task 3: Strategy list page — show strategy_type badge

**Files:**
- Modify: `web/src/routes/strategies/+page.server.ts`
- Modify: `web/src/routes/strategies/+page.svelte`

- [ ] **Step 1: Add strategy_type to the load query**

In `web/src/routes/strategies/+page.server.ts`, update the select:
```typescript
const result = await db.query(
    `select id, name, wasm_key, strategy_type, created_at
     from strategies
     where user_id = $1
     order by created_at desc`,
    [locals.user!.id]
)
```

- [ ] **Step 2: Show strategy_type in the list**

In `web/src/routes/strategies/+page.svelte`, update the badge line to show `RL` or `WASM ready`:
```svelte
<span class="badge {s.strategy_type === 'rl' ? 'rl' : s.wasm_key ? 'ready' : 'draft'}">
    {s.strategy_type === 'rl' ? 'RL' : s.wasm_key ? 'WASM ready' : 'No WASM'}
</span>
```

Add the `.rl` badge style alongside existing badge styles in the `<style>` block:
```css
.badge.rl { background: #7c3aed22; color: #a78bfa; }
```

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/strategies/+page.server.ts web/src/routes/strategies/+page.svelte
git commit -m "Show RL badge on strategy list"
```

---

### Task 4: New strategy choice screen

**Files:**
- Modify: `web/src/routes/strategies/new/+page.svelte`

- [ ] **Step 1: Replace the current page with a choice screen**

Replace the full content of `web/src/routes/strategies/new/+page.svelte`:
```svelte
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
```

- [ ] **Step 2: Move existing rule builder page to /strategies/new/rules**

```bash
mkdir -p web/src/routes/strategies/new/rules
cp web/src/routes/strategies/new/+page.server.ts web/src/routes/strategies/new/rules/+page.server.ts
cp web/src/routes/strategies/new/+page.svelte web/src/routes/strategies/new/rules/+page.svelte
# Then overwrite new/+page.svelte with the choice screen above (already done in step 1)
# Remove the old server file since new/+page.svelte no longer submits a form
rm web/src/routes/strategies/new/+page.server.ts
```

Update the `action` in `web/src/routes/strategies/new/rules/+page.svelte` — the form action doesn't need changing since it posts to the same route. Verify the `<form method="POST" use:enhance>` in the rules page still works by checking that `web/src/routes/strategies/new/rules/+page.server.ts` exists.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/strategies/new/+page.svelte web/src/routes/strategies/new/rules/
git commit -m "Split new strategy into rules vs learn choice screen"
```

---

### Task 5: RL config form — server action

**Files:**
- Create: `web/src/routes/strategies/new/rl/+page.server.ts`

- [ ] **Step 1: Write the server action**

`web/src/routes/strategies/new/rl/+page.server.ts`:
```typescript
import { db } from '$lib/server/db'
import { redirect, fail } from '@sveltejs/kit'
import type { Actions } from './$types'
import type { RLConfig } from '$lib/types/rl'

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const rl_config_raw = form.get('rl_config')?.toString()

		if (!name) return fail(400, { error: 'Name is required' })
		if (!rl_config_raw) return fail(400, { error: 'RL config is required' })

		let rl_config: RLConfig
		try {
			rl_config = JSON.parse(rl_config_raw)
		} catch {
			return fail(400, { error: 'Invalid RL config' })
		}

		if (!rl_config.train_from || !rl_config.train_to) {
			return fail(400, { error: 'Training date range is required' })
		}

		const stratResult = await db.query(
			`insert into strategies (user_id, name, strategy_type, rl_config)
			 values ($1, $2, 'rl', $3) returning id`,
			[locals.user!.id, name, rl_config]
		)
		const strategyId = stratResult.rows[0].id

		await db.query(
			`insert into rl_jobs (strategy_id) values ($1)`,
			[strategyId]
		)

		redirect(302, `/strategies/${strategyId}`)
	},
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/strategies/new/rl/+page.server.ts
git commit -m "Add RL strategy server action"
```

---

### Task 6: RL config form — UI

**Files:**
- Create: `web/src/routes/strategies/new/rl/+page.svelte`

- [ ] **Step 1: Write the page**

`web/src/routes/strategies/new/rl/+page.svelte`:
```svelte
<script lang="ts">
	import { enhance } from '$app/forms'
	import type { RLConfig, RLConstraint, RLReward } from '$lib/types/rl'
	import type { Indicator } from '$lib/types/rules'

	let { form } = $props()

	const indicatorNames = ['rsi', 'sma', 'ema', 'vwap', 'macd', 'bb'] as const

	function defaultIndicator(name: string): Indicator {
		if (name === 'macd') return { name: 'macd', component: 'macd', fast: 12, slow: 26, signal_period: 9 }
		if (name === 'bb') return { name: 'bb', component: 'upper', period: 20 }
		if (name === 'vwap') return { name: 'vwap' }
		return { name: name as any, period: 14 }
	}

	let reward: RLReward = $state('pnl')
	let allow_short = $state(false)
	let lookback_candles = $state(20)
	let train_from = $state('')
	let train_to = $state('')
	let selectedIndicators: string[] = $state(['rsi', 'sma'])
	let constraints: RLConstraint[] = $state([])

	function toggleIndicator(name: string) {
		if (selectedIndicators.includes(name)) {
			selectedIndicators = selectedIndicators.filter(n => n !== name)
		} else {
			selectedIndicators = [...selectedIndicators, name]
		}
	}

	function addConstraint(type: RLConstraint['type']) {
		const defaults: Record<RLConstraint['type'], RLConstraint> = {
			max_holding_days: { type: 'max_holding_days', value: 5 },
			max_trades_per_month: { type: 'max_trades_per_month', value: 10 },
		}
		constraints = [...constraints, defaults[type]]
	}

	function removeConstraint(i: number) {
		constraints = constraints.filter((_, idx) => idx !== i)
	}

	function updateConstraintValue(i: number, value: number) {
		constraints = constraints.map((c, idx) => idx === i ? { ...c, value } : c)
	}

	let rlConfig = $derived<RLConfig>({
		reward,
		constraints,
		indicators: selectedIndicators.map(defaultIndicator),
		lookback_candles,
		allow_short,
		train_from,
		train_to,
	})
</script>

<div class="header">
	<a href="/strategies/new" class="back">← New strategy</a>
	<h1>Learn strategy</h1>
</div>

<form method="POST" use:enhance class="form">
	{#if form?.error}
		<p class="error">{form.error}</p>
	{/if}

	<div class="field">
		<label for="name">Name</label>
		<input id="name" name="name" type="text" placeholder="e.g. RSI momentum RL" required />
	</div>

	<div class="field">
		<label>Reward objective</label>
		<div class="radio-group">
			{#each [['pnl', 'Maximize PnL'], ['sharpe', 'Maximize Sharpe ratio'], ['min_drawdown', 'Minimize drawdown']] as [val, label]}
				<label class="radio">
					<input type="radio" bind:group={reward} value={val} />
					{label}
				</label>
			{/each}
		</div>
	</div>

	<div class="field">
		<label>Indicators (features)</label>
		<div class="indicator-grid">
			{#each indicatorNames as n}
				<button type="button"
					class="ind-btn"
					class:selected={selectedIndicators.includes(n)}
					onclick={() => toggleIndicator(n)}>
					{n.toUpperCase()}
				</button>
			{/each}
		</div>
		<p class="hint">Selected indicators + a {lookback_candles}-candle OHLCV window form the state.</p>
	</div>

	<div class="field">
		<label>OHLCV lookback candles</label>
		<input type="number" min="5" max="100" bind:value={lookback_candles} class="num-input" />
	</div>

	<div class="field">
		<label>Constraints</label>
		<div class="constraints">
			{#each constraints as c, i}
				<div class="constraint-row">
					<span class="constraint-label">
						{c.type === 'max_holding_days' ? 'Max holding days' : 'Max trades/month'}
					</span>
					<input type="number" min="1" value={c.value}
						oninput={(e) => updateConstraintValue(i, +(e.target as HTMLInputElement).value)}
						class="num-input" />
					<button type="button" class="del" onclick={() => removeConstraint(i)}>✕</button>
				</div>
			{/each}
		</div>
		<div class="constraint-add">
			<button type="button" class="btn-add" onclick={() => addConstraint('max_holding_days')}>+ Max holding days</button>
			<button type="button" class="btn-add" onclick={() => addConstraint('max_trades_per_month')}>+ Max trades/month</button>
		</div>
	</div>

	<div class="field">
		<label class="checkbox-label">
			<input type="checkbox" bind:checked={allow_short} />
			Allow short positions
		</label>
	</div>

	<div class="field">
		<label>Training data range</label>
		<div class="date-row">
			<input type="date" bind:value={train_from} required />
			<span>→</span>
			<input type="date" bind:value={train_to} required />
		</div>
	</div>

	<input type="hidden" name="rl_config" value={JSON.stringify(rlConfig)} />

	<div class="footer">
		<a href="/strategies/new" class="btn-secondary">Cancel</a>
		<button type="submit" class="btn-primary">Start training</button>
	</div>
</form>

<style>
	.header { margin-bottom: 24px; }
	.back { color: var(--text-muted); font-size: 0.8rem; text-decoration: none; }
	.back:hover { color: var(--text); }
	h1 { font-size: 1.25rem; font-weight: 600; margin: 8px 0 0; }
	.form { display: flex; flex-direction: column; gap: 20px; max-width: 560px; }
	.field { display: flex; flex-direction: column; gap: 8px; }
	label { color: var(--text-muted); font-size: 0.8rem; font-weight: 500; }
	.hint { color: var(--text-muted); font-size: 0.75rem; margin: 0; }

	input[type='text'], input[type='date'] {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-family: 'Inter', sans-serif;
		font-size: 0.875rem;
		outline: none;
		padding: 8px 10px;
	}
	input[type='text']:focus, input[type='date']:focus { border-color: var(--accent); }

	.num-input {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text);
		font-size: 0.875rem;
		padding: 6px 10px;
		width: 80px;
	}

	.radio-group { display: flex; flex-direction: column; gap: 8px; }
	.radio { align-items: center; color: var(--text); display: flex; font-size: 0.875rem; font-weight: 400; gap: 8px; }

	.indicator-grid { display: flex; flex-wrap: wrap; gap: 8px; }
	.ind-btn {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.8rem;
		padding: 4px 12px;
	}
	.ind-btn.selected { border-color: var(--accent); color: var(--accent); }

	.constraints { display: flex; flex-direction: column; gap: 8px; }
	.constraint-row { align-items: center; display: flex; gap: 10px; }
	.constraint-label { color: var(--text); font-size: 0.85rem; min-width: 160px; }
	.del { background: none; border: none; color: var(--text-muted); cursor: pointer; font-size: 0.75rem; }
	.del:hover { color: var(--red-muted); }

	.constraint-add { display: flex; gap: 8px; }
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

	.checkbox-label { align-items: center; color: var(--text); display: flex; font-size: 0.875rem; font-weight: 400; gap: 8px; }

	.date-row { align-items: center; display: flex; gap: 10px; }
	.date-row span { color: var(--text-muted); }

	.footer { display: flex; gap: 12px; justify-content: flex-end; padding-top: 8px; }
	.btn-primary {
		background: var(--accent); border: none; border-radius: 6px; color: #000;
		cursor: pointer; font-family: 'Inter', sans-serif; font-size: 0.875rem;
		font-weight: 500; padding: 8px 20px;
	}
	.btn-primary:hover { background: var(--accent-hover); }
	.btn-secondary {
		border: 1px solid var(--border); border-radius: 6px; color: var(--text-muted);
		font-size: 0.875rem; padding: 8px 20px; text-decoration: none;
	}
	.btn-secondary:hover { color: var(--text); }
	.error { background: var(--red-bg); border-radius: 6px; color: var(--red-muted); font-size: 0.85rem; padding: 10px 14px; }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/strategies/new/rl/+page.svelte
git commit -m "Add RL config UI"
```

---

### Task 7: Strategy detail page — show RL status and summary

**Files:**
- Modify: `web/src/routes/strategies/[id]/+page.server.ts`
- Modify: `web/src/routes/strategies/[id]/+page.svelte`

- [ ] **Step 1: Add rl_config, rl_summary, rl_job status to load**

In `web/src/routes/strategies/[id]/+page.server.ts`, update the strategy query:
```typescript
const stratResult = await db.query(
    `select s.id, s.name, s.source_key, s.wasm_key, s.created_at,
            s.strategy_type, s.rl_config, s.rl_summary,
            j.status as build_status, j.error as build_error
     from strategies s
     left join lateral (
       select status, error from build_jobs
       where strategy_id = s.id
       order by created_at desc
       limit 1
     ) j on true
     where s.id = $1 and s.user_id = $2`,
    [params.id, locals.user!.id]
)
```

Also add RL job status lookup after the existing queries:
```typescript
const rlJobResult = await db.query(
    `select status, error from rl_jobs where strategy_id = $1 order by created_at desc limit 1`,
    [params.id]
)

return {
    strategy: {
        ...stratResult.rows[0],
        rl_job: rlJobResult.rows[0] ?? null,
    },
    runs: runsResult.rows,
    policies: policiesResult.rows,
}
```

- [ ] **Step 2: Show RL section in the detail page**

In `web/src/routes/strategies/[id]/+page.svelte`, find where the build status is shown and add an RL section for `strategy_type === 'rl'`. Add after the strategy header / before the runs section:

```svelte
{#if data.strategy.strategy_type === 'rl'}
    <section class="rl-section">
        <h2>RL Training</h2>

        {#if data.strategy.rl_job?.status === 'pending' || data.strategy.rl_job?.status === 'training'}
            <p class="status-badge training">Training in progress…</p>
        {:else if data.strategy.rl_job?.status === 'failed'}
            <p class="status-badge failed">Training failed: {data.strategy.rl_job.error}</p>
        {:else if data.strategy.rl_summary}
            {@const summary = data.strategy.rl_summary}
            <div class="summary-grid">
                <div class="summary-item">
                    <span class="summary-label">Episodes</span>
                    <span class="summary-value">{summary.training_episodes}</span>
                </div>
                <div class="summary-item">
                    <span class="summary-label">Final reward</span>
                    <span class="summary-value">{summary.final_reward.toFixed(4)}</span>
                </div>
            </div>

            <h3>Feature importance</h3>
            <div class="feature-list">
                {#each summary.feature_importance as f}
                    <div class="feature-row">
                        <span class="feature-name">{f.name}</span>
                        <div class="feature-bar-wrap">
                            <div class="feature-bar" style="width: {(f.importance * 100).toFixed(1)}%"></div>
                        </div>
                        <span class="feature-pct">{(f.importance * 100).toFixed(1)}%</span>
                    </div>
                {/each}
            </div>

            <h3>Approximate rules</h3>
            <pre class="rules-pre">{summary.approximate_rules}</pre>
        {:else}
            <p class="status-badge pending">Waiting to start…</p>
        {/if}
    </section>
{/if}
```

Add styles for the new elements in the page's `<style>` block:
```css
.rl-section { margin-bottom: 32px; }
.rl-section h2 { font-size: 1rem; font-weight: 600; margin: 0 0 12px; }
.rl-section h3 { color: var(--text-muted); font-size: 0.8rem; font-weight: 600; margin: 16px 0 8px; text-transform: uppercase; letter-spacing: 0.05em; }

.status-badge { border-radius: 4px; display: inline-block; font-size: 0.85rem; padding: 6px 12px; }
.status-badge.training { background: #1e40af22; color: #60a5fa; }
.status-badge.failed { background: var(--red-bg); color: var(--red-muted); }
.status-badge.pending { background: var(--bg-surface); color: var(--text-muted); }

.summary-grid { display: flex; gap: 16px; }
.summary-item { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; padding: 12px 16px; }
.summary-label { color: var(--text-muted); display: block; font-size: 0.75rem; margin-bottom: 4px; }
.summary-value { font-size: 1.25rem; font-weight: 600; }

.feature-list { display: flex; flex-direction: column; gap: 6px; }
.feature-row { align-items: center; display: flex; gap: 10px; }
.feature-name { color: var(--text-muted); font-size: 0.8rem; min-width: 120px; }
.feature-bar-wrap { background: var(--bg-surface); border-radius: 3px; flex: 1; height: 6px; }
.feature-bar { background: var(--accent); border-radius: 3px; height: 100%; }
.feature-pct { color: var(--text-muted); font-size: 0.75rem; min-width: 36px; text-align: right; }

.rules-pre { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; color: var(--text); font-size: 0.8rem; line-height: 1.6; overflow-x: auto; padding: 12px; white-space: pre-wrap; }
```

- [ ] **Step 3: Commit**

```bash
git add "web/src/routes/strategies/[id]/+page.server.ts" "web/src/routes/strategies/[id]/+page.svelte"
git commit -m "Show RL training status and summary on strategy detail page"
```

---

### Task 8: Builder — add ndarray/rand dependencies

**Files:**
- Modify: `builder/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Add to the `[dependencies]` section of `builder/Cargo.toml`:
```toml
ndarray = "0.16"
rand = "0.9"
rand_distr = "0.5"
```

- [ ] **Step 2: Verify it compiles**

```bash
cd builder && cargo check 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add builder/Cargo.toml
git commit -m "Add ndarray and rand deps to builder for PPO"
```

---

### Task 9: Feature computation

**Files:**
- Create: `builder/src/rl/features.rs`
- Create: `builder/src/rl/mod.rs` (stub, will be expanded in Task 10)

- [ ] **Step 1: Create the rl module stub**

`builder/src/rl/mod.rs`:
```rust
pub mod features;
pub mod distill;
pub mod train;
```

- [ ] **Step 2: Write feature computation**

`builder/src/rl/features.rs`:
```rust
use ndarray::{Array1, Array2};

/// Candle data for one instrument over the training window.
pub struct Candles {
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub closes: Vec<f64>,
    pub volumes: Vec<f64>,
}

/// Which indicators to include in the state vector.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "name", rename_all = "lowercase")]
pub enum IndicatorSpec {
    Rsi { period: usize },
    Sma { period: usize },
    Ema { period: usize },
    Vwap,
    Macd { component: String, fast: usize, slow: usize, signal_period: usize },
    Bb { component: String, period: usize },
}

impl IndicatorSpec {
    pub fn name(&self) -> String {
        match self {
            Self::Rsi { period } => format!("rsi_{}", period),
            Self::Sma { period } => format!("sma_{}", period),
            Self::Ema { period } => format!("ema_{}", period),
            Self::Vwap => "vwap".into(),
            Self::Macd { component, fast, slow, signal_period } =>
                format!("macd_{}_{}_{}", fast, slow, signal_period) + "_" + component,
            Self::Bb { component, period } => format!("bb_{}_{}", period, component),
        }
    }
}

/// Compute indicator series for all specs.
/// Returns a Vec of (name, values) where values.len() == candles.closes.len().
pub fn compute_indicators(candles: &Candles, specs: &[IndicatorSpec]) -> Vec<(String, Vec<f64>)> {
    use indicators::{rsi::rsi, sma::sma, ema::ema, vwap::vwap, macd::macd, bb::bb};
    let mut out = vec![];
    for spec in specs {
        let series: Vec<f64> = match spec {
            IndicatorSpec::Rsi { period } => rsi(&candles.closes, *period),
            IndicatorSpec::Sma { period } => sma(&candles.closes, *period),
            IndicatorSpec::Ema { period } => ema(&candles.closes, *period),
            IndicatorSpec::Vwap => vwap(&candles.closes, &candles.volumes),
            IndicatorSpec::Macd { component, fast, slow, signal_period } => {
                let (m, s, h) = macd(&candles.closes, *fast, *slow, *signal_period);
                match component.as_str() {
                    "signal" => s,
                    "histogram" => h,
                    _ => m,
                }
            },
            IndicatorSpec::Bb { component, period } => {
                let (u, mid, l) = bb(&candles.closes, *period);
                match component.as_str() {
                    "middle" => mid,
                    "lower" => l,
                    _ => u,
                }
            },
        };
        out.push((spec.name(), series));
    }
    out
}

/// Build the state matrix: rows = timesteps, cols = indicator values + OHLCV window.
/// Returns (state_matrix, feature_names).
/// Rows where any indicator is NaN are skipped (warm-up period).
pub fn build_state_matrix(
    candles: &Candles,
    indicator_series: &[(String, Vec<f64>)],
    lookback: usize,
) -> (Array2<f64>, Vec<String>) {
    let n = candles.closes.len();
    let ind_count = indicator_series.len();
    // OHLCV × lookback
    let ohlcv_count = 5 * lookback;
    let state_dim = ind_count + ohlcv_count;

    let mut feature_names: Vec<String> = indicator_series.iter().map(|(n, _)| n.clone()).collect();
    for lag in 1..=lookback {
        for col in ["open", "high", "low", "close", "volume"] {
            feature_names.push(format!("{}_t-{}", col, lag));
        }
    }

    let mut rows: Vec<Array1<f64>> = vec![];

    for i in lookback..n {
        // check no indicator is NaN at this step
        let ind_vals: Vec<f64> = indicator_series.iter().map(|(_, v)| v[i]).collect();
        if ind_vals.iter().any(|v| v.is_nan()) { continue; }

        let mut state = Array1::zeros(state_dim);
        for (j, &v) in ind_vals.iter().enumerate() {
            state[j] = v;
        }
        let mut off = ind_count;
        for lag in 1..=lookback {
            let t = i - lag;
            state[off] = candles.opens[t];   off += 1;
            state[off] = candles.highs[t];   off += 1;
            state[off] = candles.lows[t];    off += 1;
            state[off] = candles.closes[t];  off += 1;
            state[off] = candles.volumes[t]; off += 1;
        }
        rows.push(state);
    }

    if rows.is_empty() {
        return (Array2::zeros((0, state_dim)), feature_names);
    }

    let nrows = rows.len();
    let mut mat = Array2::zeros((nrows, state_dim));
    for (i, row) in rows.into_iter().enumerate() {
        mat.row_mut(i).assign(&row);
    }
    (mat, feature_names)
}

/// Normalise each column to zero mean, unit std (in-place).
pub fn normalise(mat: &mut Array2<f64>) {
    let ncols = mat.ncols();
    for j in 0..ncols {
        let col: Vec<f64> = mat.column(j).to_vec();
        let mean = col.iter().sum::<f64>() / col.len() as f64;
        let std = (col.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / col.len() as f64).sqrt();
        let std = if std < 1e-8 { 1.0 } else { std };
        for v in mat.column_mut(j).iter_mut() {
            *v = (*v - mean) / std;
        }
    }
}
```

- [ ] **Step 3: Add indicators crate dependency to builder**

In `builder/Cargo.toml`, add:
```toml
indicators = { path = "../rust/indicators" }
```

- [ ] **Step 4: Verify compile**

```bash
cd builder && cargo check 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add builder/src/rl/ builder/Cargo.toml
git commit -m "Add RL feature computation (indicators + OHLCV state matrix)"
```

---

### Task 10: PPO trainer

**Files:**
- Create: `builder/src/rl/train.rs`

- [ ] **Step 1: Write the PPO trainer**

`builder/src/rl/train.rs`:
```rust
use ndarray::{Array1, Array2, s};
use rand::Rng;
use rand_distr::{Normal, Distribution};

/// Small MLP: input → hidden1 → hidden2 → output.
/// All weights stored as flat Vecs (row-major).
pub struct MLP {
    pub w1: Vec<f64>,  // hidden_size × input_size
    pub b1: Vec<f64>,  // hidden_size
    pub w2: Vec<f64>,  // hidden_size × hidden_size
    pub b2: Vec<f64>,  // hidden_size
    pub w_mean: Vec<f64>,  // action_size × hidden_size
    pub b_mean: Vec<f64>,  // action_size
    pub log_std: Vec<f64>, // action_size (learnable, not input-dependent)
    pub input_size: usize,
    pub hidden_size: usize,
    pub action_size: usize,
}

impl MLP {
    pub fn new(input_size: usize, hidden_size: usize, action_size: usize) -> Self {
        let mut rng = rand::rng();
        let scale = (2.0 / input_size as f64).sqrt();
        let normal = Normal::new(0.0, scale).unwrap();
        let init = |n: usize| -> Vec<f64> { (0..n).map(|_| normal.sample(&mut rng)).collect() };
        Self {
            w1: init(hidden_size * input_size),
            b1: vec![0.0; hidden_size],
            w2: init(hidden_size * hidden_size),
            b2: vec![0.0; hidden_size],
            w_mean: init(action_size * hidden_size),
            b_mean: vec![0.0; action_size],
            log_std: vec![-0.5; action_size],
            input_size,
            hidden_size,
            action_size,
        }
    }

    fn tanh(x: f64) -> f64 { x.tanh() }

    fn matmul_bias(w: &[f64], b: &[f64], x: &[f64], rows: usize, cols: usize) -> Vec<f64> {
        (0..rows).map(|i| {
            b[i] + (0..cols).map(|j| w[i * cols + j] * x[j]).sum::<f64>()
        }).collect()
    }

    pub fn forward(&self, x: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let h1: Vec<f64> = Self::matmul_bias(&self.w1, &self.b1, x, self.hidden_size, self.input_size)
            .iter().map(|&v| Self::tanh(v)).collect();
        let h2: Vec<f64> = Self::matmul_bias(&self.w2, &self.b2, &h1, self.hidden_size, self.hidden_size)
            .iter().map(|&v| Self::tanh(v)).collect();
        let mean = Self::matmul_bias(&self.w_mean, &self.b_mean, &h2, self.action_size, self.hidden_size);
        let std: Vec<f64> = self.log_std.iter().map(|&ls| ls.exp().max(0.01)).collect();
        (mean, std)
    }

    /// Sample action from Gaussian policy.
    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (Vec<f64>, f64) {
        let (mean, std) = self.forward(x);
        let normal = Normal::new(mean[0], std[0]).unwrap();
        let raw = normal.sample(rng);
        let action = if allow_short {
            raw.clamp(-1.0, 1.0)
        } else {
            raw.clamp(0.0, 1.0)
        };
        // log prob of the raw sample under Gaussian
        let log_prob = -0.5 * ((raw - mean[0]) / std[0]).powi(2) - std[0].ln() - (2.0 * std::f64::consts::PI).sqrt().ln();
        (vec![action], log_prob)
    }

    /// All parameters as a flat Vec (for gradient updates).
    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        p.extend_from_slice(&self.w1); p.extend_from_slice(&self.b1);
        p.extend_from_slice(&self.w2); p.extend_from_slice(&self.b2);
        p.extend_from_slice(&self.w_mean); p.extend_from_slice(&self.b_mean);
        p.extend_from_slice(&self.log_std);
        p
    }

    /// Update parameters from flat gradient Vec using Adam.
    pub fn apply_adam(&mut self, grads: &[f64], m: &mut Vec<f64>, v: &mut Vec<f64>, t: usize, lr: f64) {
        let beta1 = 0.9f64;
        let beta2 = 0.999f64;
        let eps = 1e-8f64;
        let bc1 = 1.0 - beta1.powi(t as i32);
        let bc2 = 1.0 - beta2.powi(t as i32);

        let mut params = self.params();
        for i in 0..params.len() {
            m[i] = beta1 * m[i] + (1.0 - beta1) * grads[i];
            v[i] = beta2 * v[i] + (1.0 - beta2) * grads[i].powi(2);
            let m_hat = m[i] / bc1;
            let v_hat = v[i] / bc2;
            params[i] -= lr * m_hat / (v_hat.sqrt() + eps);
        }
        self.load_params(&params);
    }

    pub fn load_params(&mut self, p: &[f64]) {
        let mut off = 0;
        let copy = |dst: &mut Vec<f64>, p: &[f64], off: &mut usize| {
            let n = dst.len(); dst.copy_from_slice(&p[*off..*off+n]); *off += n;
        };
        copy(&mut self.w1, p, &mut off); copy(&mut self.b1, p, &mut off);
        copy(&mut self.w2, p, &mut off); copy(&mut self.b2, p, &mut off);
        copy(&mut self.w_mean, p, &mut off); copy(&mut self.b_mean, p, &mut off);
        copy(&mut self.log_std, p, &mut off);
    }

    pub fn param_count(&self) -> usize { self.params().len() }
}

pub struct TrainConfig {
    pub max_episodes: usize,
    pub episode_steps: usize,  // candles per episode
    pub lr: f64,
    pub gamma: f64,            // discount factor
    pub clip_eps: f64,         // PPO clip epsilon
    pub allow_short: bool,
    pub reward_type: String,   // "pnl" | "sharpe" | "min_drawdown"
    pub penalty_holding_days: Option<f64>,   // penalty weight if > max
    pub max_holding_days: Option<usize>,
    pub penalty_trades_per_month: Option<f64>,
    pub max_trades_per_month: Option<usize>,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            max_episodes: 500,
            episode_steps: 200,
            lr: 3e-4,
            gamma: 0.99,
            clip_eps: 0.2,
            allow_short: false,
            reward_type: "pnl".into(),
            penalty_holding_days: None,
            max_holding_days: None,
            penalty_trades_per_month: None,
            max_trades_per_month: None,
        }
    }
}

struct Trajectory {
    states: Vec<Vec<f64>>,
    actions: Vec<f64>,
    log_probs: Vec<f64>,
    rewards: Vec<f64>,
    returns: Vec<f64>,
}

fn compute_returns(rewards: &[f64], gamma: f64) -> Vec<f64> {
    let mut returns = vec![0.0; rewards.len()];
    let mut g = 0.0;
    for i in (0..rewards.len()).rev() {
        g = rewards[i] + gamma * g;
        returns[i] = g;
    }
    returns
}

fn step_reward(
    position: f64,   // current position size
    prev_price: f64,
    curr_price: f64,
    trade_count: usize,
    holding_candles: usize,
    config: &TrainConfig,
) -> f64 {
    let pnl = position * (curr_price - prev_price) / prev_price;
    let mut reward = match config.reward_type.as_str() {
        "pnl" => pnl,
        "sharpe" => pnl,          // accumulate for Sharpe normalisation at episode end
        "min_drawdown" => pnl,    // same
        _ => pnl,
    };

    // constraint penalties
    if let (Some(max_days), Some(w)) = (config.max_holding_days, config.penalty_holding_days) {
        let holding_days = holding_candles as f64 / 26.0; // rough: 26 candles/day for 15min
        if holding_days > max_days as f64 {
            reward -= w * (holding_days - max_days as f64);
        }
    }
    if let (Some(max_trades), Some(w)) = (config.max_trades_per_month, config.penalty_trades_per_month) {
        let monthly_rate = trade_count as f64 / 20.0; // rough: 20 trading days/month
        if monthly_rate > max_trades as f64 {
            reward -= w * (monthly_rate - max_trades as f64);
        }
    }
    reward
}

/// Run one episode on a slice of the state matrix (random start offset).
fn rollout(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,  // index into closes corresponding to states row 0
    config: &TrainConfig,
    rng: &mut impl Rng,
) -> Trajectory {
    let n = states.nrows().min(config.episode_steps);
    let mut position = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;

    let mut traj = Trajectory {
        states: vec![], actions: vec![], log_probs: vec![],
        rewards: vec![], returns: vec![],
    };

    for t in 0..n {
        let state: Vec<f64> = states.row(t).to_vec();
        let (action, lp) = net.sample_action(&state, rng, config.allow_short);
        let a = action[0];

        let ci = state_offset + t;
        let prev_price = if ci > 0 { closes[ci - 1] } else { closes[ci] };
        let curr_price = closes[ci];

        if (a - position).abs() > 0.05 { trades += 1; holding = 0; } else { holding += 1; }
        position = a;

        let r = step_reward(position, prev_price, curr_price, trades, holding, config);

        traj.states.push(state);
        traj.actions.push(a);
        traj.log_probs.push(lp);
        traj.rewards.push(r);
    }

    traj.returns = compute_returns(&traj.rewards, config.gamma);
    traj
}

/// Estimate policy gradient via REINFORCE with PPO clip (single-step approx).
/// Returns gradient vector aligned with net.params().
fn ppo_grad(net: &MLP, traj: &Trajectory, clip_eps: f64) -> (Vec<f64>, f64) {
    let mut grads = vec![0.0f64; net.param_count()];
    let mut total_loss = 0.0f64;
    let eps = 1e-6;

    // numerical gradient (finite differences) — simple but sufficient for small nets
    let params = net.params();
    let h = 1e-4;

    for i in 0..params.len() {
        let mut p_plus = params.clone();
        let mut p_minus = params.clone();
        p_plus[i] += h;
        p_minus[i] -= h;

        let mut net_plus = MLP::new(net.input_size, net.hidden_size, net.action_size);
        net_plus.load_params(&p_plus);
        let mut net_minus = MLP::new(net.input_size, net.hidden_size, net.action_size);
        net_minus.load_params(&p_minus);

        let mut loss_plus = 0.0f64;
        let mut loss_minus = 0.0f64;

        for (t, state) in traj.states.iter().enumerate() {
            let ret = traj.returns[t];
            let old_lp = traj.log_probs[t];
            let action = traj.actions[t];

            let (mean_p, std_p) = net_plus.forward(state);
            let lp_plus = -0.5 * ((action - mean_p[0]) / (std_p[0] + eps)).powi(2) - (std_p[0] + eps).ln();
            let ratio_plus = (lp_plus - old_lp).exp().clamp(1.0 - clip_eps, 1.0 + clip_eps);
            loss_plus -= ratio_plus * ret;

            let (mean_m, std_m) = net_minus.forward(state);
            let lp_minus = -0.5 * ((action - mean_m[0]) / (std_m[0] + eps)).powi(2) - (std_m[0] + eps).ln();
            let ratio_minus = (lp_minus - old_lp).exp().clamp(1.0 - clip_eps, 1.0 + clip_eps);
            loss_minus -= ratio_minus * ret;
        }

        grads[i] = (loss_plus - loss_minus) / (2.0 * h);
        if i == 0 { total_loss = loss_plus; }
    }

    (grads, total_loss / traj.states.len() as f64)
}

pub struct TrainResult {
    pub net: MLP,
    pub final_reward: f64,
    pub episodes: usize,
}

pub fn train(
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
    config: &TrainConfig,
) -> TrainResult {
    let input_size = states.ncols();
    let hidden_size = 64;
    let action_size = 1;
    let mut net = MLP::new(input_size, hidden_size, action_size);
    let param_count = net.param_count();
    let mut m = vec![0.0f64; param_count];
    let mut v = vec![0.0f64; param_count];
    let mut rng = rand::rng();
    let mut final_reward = 0.0f64;
    let mut adam_t = 0usize;

    for ep in 0..config.max_episodes {
        let traj = rollout(&net, states, closes, state_offset, config, &mut rng);
        final_reward = traj.rewards.iter().sum::<f64>() / traj.rewards.len() as f64;

        let (grads, _loss) = ppo_grad(&net, &traj, config.clip_eps);
        adam_t += 1;
        net.apply_adam(&grads, &mut m, &mut v, adam_t, config.lr);

        if ep % 100 == 0 {
            eprintln!("rl train: episode {}/{} avg_reward={:.4}", ep, config.max_episodes, final_reward);
        }
    }

    TrainResult { net, final_reward, episodes: config.max_episodes }
}

/// Serialise network weights to bytes (little-endian f64).
pub fn weights_to_bytes(net: &MLP) -> Vec<u8> {
    net.params().iter().flat_map(|&v| v.to_le_bytes()).collect()
}
```

- [ ] **Step 2: Verify compile**

```bash
cd builder && cargo check 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "Add PPO trainer (MLP policy + Adam + finite-difference gradient)"
```

---

### Task 11: Policy distillation and feature importance

**Files:**
- Create: `builder/src/rl/distill.rs`

- [ ] **Step 1: Write distillation**

`builder/src/rl/distill.rs`:
```rust
use ndarray::Array2;
use crate::rl::train::MLP;

/// Compute feature importance by input perturbation.
/// For each feature, zero it out and measure the average change in action.
pub fn feature_importance(net: &MLP, states: &Array2<f64>) -> Vec<f64> {
    let n = states.nrows();
    let d = states.ncols();
    if n == 0 { return vec![0.0; d]; }

    let baseline: Vec<f64> = (0..n)
        .map(|i| { let s = states.row(i).to_vec(); let (m, _) = net.forward(&s); m[0] })
        .collect();

    (0..d).map(|j| {
        let perturbed: Vec<f64> = (0..n).map(|i| {
            let mut s = states.row(i).to_vec();
            s[j] = 0.0;
            let (m, _) = net.forward(&s);
            m[0]
        }).collect();
        let diff: f64 = baseline.iter().zip(perturbed.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>() / n as f64;
        diff
    }).collect()
}

/// Normalise importances so they sum to 1.
pub fn normalise_importance(imp: &[f64]) -> Vec<f64> {
    let total: f64 = imp.iter().sum();
    if total < 1e-10 { return vec![1.0 / imp.len() as f64; imp.len()]; }
    imp.iter().map(|v| v / total).collect()
}

/// Simple decision tree node for policy distillation.
enum TreeNode {
    Leaf { action_mean: f64 },
    Split { feature: usize; threshold: f64; left: Box<TreeNode>; right: Box<TreeNode> },
}

impl TreeNode {
    fn predict(&self, x: &[f64]) -> f64 {
        match self {
            Self::Leaf { action_mean } => *action_mean,
            Self::Split { feature, threshold, left, right } => {
                if x[*feature] <= *threshold { left.predict(x) } else { right.predict(x) }
            }
        }
    }

    fn to_text(&self, feature_names: &[String], depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            Self::Leaf { action_mean } => {
                let signal = if *action_mean > 0.2 { "BUY" } else if *action_mean < -0.2 { "SELL" } else { "HOLD" };
                format!("{}→ {} (position={:.2})\n", indent, signal, action_mean)
            }
            Self::Split { feature, threshold, left, right } => {
                let fname = feature_names.get(*feature).map(|s| s.as_str()).unwrap_or("?");
                format!(
                    "{}if {} ≤ {:.4}:\n{}else:\n{}",
                    indent, fname, threshold,
                    left.to_text(feature_names, depth + 1),
                    right.to_text(feature_names, depth + 1),
                )
            }
        }
    }
}

fn best_split(
    states: &[Vec<f64>],
    actions: &[f64],
    feature: usize,
) -> (f64, f64) {
    let mut vals: Vec<f64> = states.iter().map(|s| s[feature]).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    vals.dedup();

    let mut best_thresh = vals[0];
    let mut best_loss = f64::MAX;

    for &thresh in &vals {
        let left: Vec<f64> = states.iter().zip(actions).filter(|(s, _)| s[feature] <= thresh).map(|(_, &a)| a).collect();
        let right: Vec<f64> = states.iter().zip(actions).filter(|(s, _)| s[feature] > thresh).map(|(_, &a)| a).collect();
        if left.is_empty() || right.is_empty() { continue; }
        let mse = |v: &[f64]| -> f64 {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64
        };
        let loss = mse(&left) * left.len() as f64 + mse(&right) * right.len() as f64;
        if loss < best_loss { best_loss = loss; best_thresh = thresh; }
    }
    (best_thresh, best_loss)
}

fn build_tree(states: &[Vec<f64>], actions: &[f64], depth: usize, max_depth: usize) -> TreeNode {
    let mean = actions.iter().sum::<f64>() / actions.len() as f64;
    if depth >= max_depth || actions.len() < 4 {
        return TreeNode::Leaf { action_mean: mean };
    }

    let d = states[0].len();
    let (best_feat, best_thresh) = (0..d)
        .map(|j| { let (t, l) = best_split(states, actions, j); (j, t, l) })
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
        .map(|(j, t, _)| (j, t))
        .unwrap_or((0, 0.0));

    let (left_s, left_a): (Vec<_>, Vec<_>) = states.iter().zip(actions.iter())
        .filter(|(s, _)| s[best_feat] <= best_thresh)
        .map(|(s, &a)| (s.clone(), a))
        .unzip();
    let (right_s, right_a): (Vec<_>, Vec<_>) = states.iter().zip(actions.iter())
        .filter(|(s, _)| s[best_feat] > best_thresh)
        .map(|(s, &a)| (s.clone(), a))
        .unzip();

    if left_s.is_empty() || right_s.is_empty() {
        return TreeNode::Leaf { action_mean: mean };
    }

    TreeNode::Split {
        feature: best_feat,
        threshold: best_thresh,
        left: Box::new(build_tree(&left_s, &left_a, depth + 1, max_depth)),
        right: Box::new(build_tree(&right_s, &right_a, depth + 1, max_depth)),
    }
}

/// Distil the policy into a human-readable decision tree text.
pub fn distil(net: &MLP, states: &Array2<f64>, feature_names: &[String], max_depth: usize) -> String {
    let n = states.nrows();
    if n == 0 { return "No data".into(); }

    let state_vecs: Vec<Vec<f64>> = (0..n).map(|i| states.row(i).to_vec()).collect();
    let actions: Vec<f64> = state_vecs.iter()
        .map(|s| { let (m, _) = net.forward(s); m[0] })
        .collect();

    let tree = build_tree(&state_vecs, &actions, 0, max_depth);
    tree.to_text(feature_names, 0)
}
```

- [ ] **Step 2: Verify compile**

```bash
cd builder && cargo check 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add builder/src/rl/distill.rs
git commit -m "Add policy distillation and feature importance for RL"
```

---

### Task 12: Wire rl_jobs polling into builder main loop

**Files:**
- Modify: `builder/src/main.rs`

- [ ] **Step 1: Add rl module and process_rl_job function**

At the top of `builder/src/main.rs`, after existing `use` statements, add:
```rust
mod rl;
use rl::features::{Candles, IndicatorSpec, compute_indicators, build_state_matrix, normalise};
use rl::train::{TrainConfig, train as rl_train, weights_to_bytes};
use rl::distill::{feature_importance, normalise_importance, distil};
```

Add this function before `main`:
```rust
async fn process_rl_job(
    db: &tokio_postgres::Client,
    s3: &S3Client,
    bucket: &str,
    job_id: uuid::Uuid,
    strategy_id: uuid::Uuid,
) -> Result<(), String> {
    db.execute(
        "update rl_jobs set status='training', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    // Load rl_config
    let row = db.query_one(
        "select rl_config from strategies where id=$1",
        &[&strategy_id],
    ).await.map_err(|e| e.to_string())?;

    let rl_config: serde_json::Value = row.get(0);

    let train_from: String = rl_config["train_from"].as_str().unwrap_or("").to_string();
    let train_to: String = rl_config["train_to"].as_str().unwrap_or("").to_string();
    let lookback = rl_config["lookback_candles"].as_u64().unwrap_or(20) as usize;
    let allow_short = rl_config["allow_short"].as_bool().unwrap_or(false);
    let reward_type = rl_config["reward"].as_str().unwrap_or("pnl").to_string();

    let indicator_specs: Vec<IndicatorSpec> = serde_json::from_value(
        rl_config["indicators"].clone()
    ).map_err(|e| e.to_string())?;

    // constraints
    let mut penalty_holding = None;
    let mut max_holding_days = None;
    let mut penalty_trades = None;
    let mut max_trades_per_month = None;
    if let Some(arr) = rl_config["constraints"].as_array() {
        for c in arr {
            match c["type"].as_str().unwrap_or("") {
                "max_holding_days" => {
                    max_holding_days = c["value"].as_u64().map(|v| v as usize);
                    penalty_holding = Some(0.01);
                }
                "max_trades_per_month" => {
                    max_trades_per_month = c["value"].as_u64().map(|v| v as usize);
                    penalty_trades = Some(0.01);
                }
                _ => {}
            }
        }
    }

    // Fetch candles (use NSE_E default — RL trains on one instrument for now)
    // For multi-instrument we'd average or pick the first instrument from a separate config field.
    // TODO: extend rl_config to include instrument selection.
    // For now fetch any instrument from a recent backtest run for this strategy, or error.
    let inst_row = db.query_opt(
        "select ri.security_id, ri.exchange_segment
         from backtest_run_instruments ri
         join backtest_runs r on r.id = ri.run_id
         where r.strategy_id = $1
         limit 1",
        &[&strategy_id],
    ).await.map_err(|e| e.to_string())?;

    let (security_id, exchange_segment) = match inst_row {
        Some(r) => (r.get::<_, String>(0), r.get::<_, String>(1)),
        None => return Err("No instrument found — run a backtest first to select an instrument".into()),
    };

    let candle_rows = db.query(
        "select extract(epoch from timestamp)::bigint, open::float8, high::float8,
                low::float8, close::float8, volume
         from candles
         where security_id=$1 and exchange_segment=$2 and interval='day'
         and timestamp::date between $3::text::date and $4::text::date
         order by timestamp",
        &[&security_id, &exchange_segment, &train_from, &train_to],
    ).await.map_err(|e| e.to_string())?;

    if candle_rows.is_empty() {
        return Err(format!("No daily candles found for {} {} in {} to {}", security_id, exchange_segment, train_from, train_to));
    }

    let candles = Candles {
        opens:   candle_rows.iter().map(|r| r.get::<_, f64>(1)).collect(),
        highs:   candle_rows.iter().map(|r| r.get::<_, f64>(2)).collect(),
        lows:    candle_rows.iter().map(|r| r.get::<_, f64>(3)).collect(),
        closes:  candle_rows.iter().map(|r| r.get::<_, f64>(4)).collect(),
        volumes: candle_rows.iter().map(|r| r.get::<_, i64>(5) as f64).collect(),
    };

    let indicator_series = compute_indicators(&candles, &indicator_specs);
    let (mut states, feature_names) = build_state_matrix(&candles, &indicator_series, lookback);
    normalise(&mut states);

    let cfg = TrainConfig {
        max_episodes: 500,
        episode_steps: states.nrows().min(200),
        lr: 3e-4,
        gamma: 0.99,
        clip_eps: 0.2,
        allow_short,
        reward_type,
        penalty_holding_days: penalty_holding,
        max_holding_days,
        penalty_trades_per_month: penalty_trades,
        max_trades_per_month,
    };

    let result = rl_train(&states, &candles.closes, lookback, &cfg);

    // Upload weights
    let weights = weights_to_bytes(&result.net);
    let weights_key = format!("strategies/{}/weights.bin", strategy_id);
    upload(s3, bucket, &weights_key, weights).await?;

    // Feature importance
    let raw_imp = feature_importance(&result.net, &states);
    let norm_imp = normalise_importance(&raw_imp);
    let feature_importance_json: Vec<serde_json::Value> = feature_names.iter()
        .zip(norm_imp.iter())
        .map(|(name, &imp)| serde_json::json!({ "name": name, "importance": imp }))
        .collect();

    // Distil decision tree
    let approx_rules = distil(&result.net, &states, &feature_names, 3);

    let rl_summary = serde_json::json!({
        "feature_importance": feature_importance_json,
        "approximate_rules": approx_rules,
        "training_episodes": result.episodes,
        "final_reward": result.final_reward,
    });

    db.execute(
        "update strategies set rl_summary=$1 where id=$2",
        &[&rl_summary, &strategy_id],
    ).await.map_err(|e| e.to_string())?;

    db.execute(
        "update rl_jobs set status='done', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 2: Add rl_jobs polling to the main loop**

In the `main` loop in `builder/src/main.rs`, after the existing `run_jobs` block (before `tokio::time::sleep`), add:

```rust
        // rl jobs
        let rl_rows = db.query(
            "select id, strategy_id from rl_jobs where status = 'pending' order by created_at limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in rl_rows {
            let job_id: uuid::Uuid = row.get(0);
            let strategy_id: uuid::Uuid = row.get(1);

            println!("builder: starting rl training for strategy {}", strategy_id);

            match process_rl_job(&db, &s3, &bucket, job_id, strategy_id).await {
                Ok(()) => println!("builder: rl training done {}", strategy_id),
                Err(e) => {
                    eprintln!("builder: rl training failed {}: {}", strategy_id, e);
                    db.execute(
                        "update rl_jobs set status='failed', error=$1, updated_at=now() where id=$2",
                        &[&e, &job_id],
                    ).await.ok();
                }
            }
        }
```

- [ ] **Step 3: Verify compile**

```bash
cd builder && cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add builder/src/main.rs builder/src/rl/mod.rs
git commit -m "Wire rl_jobs polling into builder — trains PPO, stores weights + summary"
```

---

### Task 13: End-to-end smoke test

- [ ] **Step 1: Start the stack**

```bash
make down-local up-local
```

- [ ] **Step 2: Create an RL strategy**

Navigate to `/strategies/new` → "Learn strategy". Fill in:
- Name: "RL test"
- Reward: Maximize PnL
- Indicators: RSI, SMA
- Lookback: 20
- Training range: 2025-01-01 → 2025-12-31
- Allow short: off
- Submit

Confirm redirect to strategy detail page with "Waiting to start…" status.

- [ ] **Step 3: Verify rl_job was created**

```bash
make logs-builder 2>/dev/null | grep "rl training"
```

Expected: `builder: starting rl training for strategy <uuid>`

Note: the job will fail with "No instrument found" if no prior backtest exists for this strategy. This is expected — instrument selection for RL training is deferred to the next plan. Create a manual strategy first, run a backtest, then create the RL strategy referencing the same instrument implicitly.

- [ ] **Step 4: Verify training completes**

```bash
make logs-builder 2>/dev/null | grep "rl training done"
```

Expected: `builder: rl training done <uuid>` after ~1-2 minutes.

- [ ] **Step 5: Verify summary appears**

Navigate to the strategy detail page. Confirm:
- Feature importance bars visible
- Approximate rules text displayed
- Episode count and final reward shown

- [ ] **Step 6: Commit nothing** — smoke test is manual only.
