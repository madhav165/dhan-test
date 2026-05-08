# RL Strategy — Design Spec

**Date:** 2026-05-08

## Goal

Let users learn trading strategies using Reinforcement Learning instead of hand-crafting rules. The user configures a reward objective, constraints, indicator/feature selection, and a training data range. The builder trains a PPO policy, produces backtest metrics and an interpretable summary, and the resulting strategy is deployed exactly like a manual one.

## User Flow

Strategies page → New strategy → choice:
- "Define rules" → existing visual rule builder
- "Learn strategy" → RL config screen → training job → strategy detail with metrics + approximate rules

Both strategy types appear in the same strategy list and support the same backtest/policy deployment flow.

## State Space

Each timestep the agent observes:
- Current values of user-selected indicators (RSI, SMA, EMA, VWAP, MACD, BB)
- A raw OHLCV lookback window (user-configurable length, default 20 candles)

## Action Space

Continuous position size in [-1, 1]: negative = short, positive = long, 0 = flat. Mapped to buy/sell/hold signal for the existing pipeline by thresholding (> 0.2 → buy, < -0.2 → sell, else hold).

## Reward & Constraints

User picks one primary reward:
- Maximize total PnL
- Maximize Sharpe ratio
- Minimize max drawdown

Constraints are additive penalties on the reward signal:
- Max holding period (penalize positions held longer than N days)
- Max trade frequency (penalize more than N trades per period)
- (extensible — new constraints are new penalty terms)

## RL Algorithm

PPO (Proximal Policy Optimization) with a small MLP policy network:
- Input: state vector (indicators + OHLCV window)
- Hidden: 2 layers × 64 units, tanh activation
- Output: mean + log_std for a Gaussian over position size (continuous action)

Training is episodic over the training date range. Each episode is one pass through the candle history. Multiple episodes until convergence or a max step budget.

## Pipeline

### Training (native Rust in builder)

1. Builder polls new `rl_jobs` queue
2. Loads candles from Postgres for the training date range
3. Computes indicator values for the selected indicators
4. Runs PPO training loop (native Rust, multi-threaded via rayon)
5. After convergence: runs policy distillation (fit shallow decision tree on trajectories) and feature importance (input perturbation)
6. Serializes trained weights to a flat f32 array → uploads to MinIO as `strategies/{id}/weights.bin`
7. Writes distilled rules + feature importance JSON to `strategies.rl_summary`
8. Queues a `build_jobs` row to compile the inference WASM with the weights embedded

### Inference (WASM)

A generic `rl_inference` WASM crate is compiled once per weights update. It contains:
- The fixed network architecture (same 2×64 MLP)
- The trained weights embedded as a static byte array
- `alloc(len)` and `run(len)` exports matching the existing interface

The builder wraps the weights into the WASM source, compiles, and uploads to MinIO. From here the strategy is identical to a manual one — the policy worker and backtester call `run()` and get back signals.

## DB Changes

```sql
-- strategy_type distinguishes manual vs rl
alter table strategies add column strategy_type text not null default 'manual';

-- rl training config (reward, constraints, indicator selection, lookback, date range)
alter table strategies add column rl_config jsonb;

-- distilled rules + feature importance from training
alter table strategies add column rl_summary jsonb;

-- rl training job queue
create table rl_jobs (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid references strategies(id) on delete cascade,
  status      text not null default 'pending', -- pending | training | done | failed
  error       text,
  created_at  timestamptz default now()
);
```

## RL Config Schema (stored in `strategies.rl_config`)

```typescript
type RLConfig = {
  reward: 'pnl' | 'sharpe' | 'min_drawdown'
  constraints: Array<
    | { type: 'max_holding_days'; value: number }
    | { type: 'max_trades_per_month'; value: number }
  >
  indicators: Indicator[]        // same Indicator type as rule builder
  lookback_candles: number       // OHLCV window length, default 20
  train_from: string             // ISO date
  train_to: string               // ISO date
}
```

## RL Summary Schema (stored in `strategies.rl_summary`)

```typescript
type RLSummary = {
  feature_importance: Array<{ name: string; importance: number }>
  approximate_rules: string      // human-readable decision tree text
  training_episodes: number
  final_reward: number
}
```

## UI

### New strategy choice screen
Two cards: "Define rules" and "Learn strategy". Clicking either routes to the respective flow.

### RL config screen (`/strategies/new/rl`)
- Reward picker (radio: Maximize PnL / Maximize Sharpe / Minimize Drawdown)
- Constraints builder (add/remove constraints with value inputs)
- Indicator selector (same indicator types as rule builder, multi-select)
- Lookback window input (number of candles)
- Training date range (from/to date pickers)
- "Start training" button → creates strategy + rl_job → redirects to strategy detail

### Strategy detail page
- If `strategy_type = 'rl'`: shows RL config summary, training status, and once done: feature importance bar chart + approximate rules text
- Backtest and policy deployment work identically to manual strategies

## Out of Scope

- GPU training
- Multiple reward objectives simultaneously
- Online learning (retraining on live data)
- Short selling (action space maps to long-only for now: position in [0, 1])
