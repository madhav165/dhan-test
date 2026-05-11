# Actor-Critic Separation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split the monolithic `MLP` (shared backbone + policy head + value head) into two fully independent neural networks: `Actor` and `Critic`.

**Architecture:** Extract a base `MLP` struct that is a pure feedforward network with a single output head. Wrap it in `Actor` (adds `continuous_action`, `action_std`, policy methods) and `Critic` (adds `forward_value`, value MSE gradient). Both networks have their own hidden-layer parameters, `Grads`, and `AdamState`. PPO now updates Actor and Critic independently.

**Tech Stack:** Rust, ndarray, rand

---

### Task 1: Refactor Base `MLP` (Pure Feedforward)

**Files:**
- Modify: `builder/src/rl/train.rs:13-411`

**Step 1: Remove value head and policy logic from `MLP`**

```rust
#[derive(Clone)]
pub struct MLP {
    pub layer_weights: Vec<Array2<f64>>,
    pub layer_biases: Vec<Array1<f64>>,
    pub w_out: Array2<f64>,
    pub b_out: Array1<f64>,
    pub input_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub activation: Activation,
    pub out_size: usize,
}
```

- Remove `w_value`, `b_value`, `continuous_action`, `action_std`
- `new` signature: `new(input_size, hidden_size, num_layers, activation, out_size)`
- `forward` returns `(Vec<Array1<f64>>, Vec<Array1<f64>>, Array1<f64>)` → pre_acts, acts, logits
- Remove `forward_full`, `forward_full_with_value`, `probs`, `sample_action`, `greedy_action`, `accumulate_grad`, `accumulate_grad_ppo`

**Step 2: Update `Grads` and `AdamState`**

Remove `w_value` and `b_value` from both structs.

**Step 3: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`
Expected: errors in downstream code (expected)

**Step 4: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "refactor(rl): extract pure MLP base struct without policy/value logic"
```

---

### Task 2: Create `Actor` and `Critic` Wrappers

**Files:**
- Modify: `builder/src/rl/train.rs`

**Step 1: Implement `Actor`**

```rust
#[derive(Clone)]
pub struct Actor {
    pub net: MLP,
    pub continuous_action: bool,
    pub action_std: f64,
}

impl Actor {
    pub fn new(input_size, hidden_size, num_layers, activation, continuous_action, action_std) -> Self
    pub fn forward_full(&self, x: &[f64]) -> (Vec<Array1<f64>>, Vec<Array1<f64>>, [f64; 3])
    pub fn probs(&self, x: &[f64], mask_sell: bool) -> [f64; 3]
    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (Action, f64)
    pub fn greedy_action(&self, x: &[f64], allow_short: bool) -> Action
    pub fn accumulate_grad(&self, x: &[f64], action: Action, advantage: f64, grads: &mut Grads)
    pub fn accumulate_grad_ppo(&self, x: &[f64], action: Action, old_log_prob: f64, advantage: f64, clip_epsilon: f64, entropy_coef: f64, grads: &mut Grads)
}
```

**Step 2: Implement `Critic`**

```rust
#[derive(Clone)]
pub struct Critic {
    pub net: MLP,
}

impl Critic {
    pub fn new(input_size, hidden_size, num_layers, activation) -> Self
    pub fn forward_value(&self, x: &[f64]) -> f64
    pub fn accumulate_grad(&self, x: &[f64], ret: f64, grads: &mut Grads)
}
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`
Expected: errors in train/eval functions (expected)

**Step 4: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "feat(rl): add Actor and Critic wrappers around independent MLPs"
```

---

### Task 3: Update Rollout and Evaluation Functions

**Files:**
- Modify: `builder/src/rl/train.rs`

**Step 1: Update `rollout` signature**

```rust
fn rollout(actor: &Actor, ...) -> (Vec<Step>, f64)
```

**Step 2: Update `rollout_ppo` signature**

```rust
fn rollout_ppo(actor: &Actor, critic: &Critic, ...) -> (Vec<TrajectoryStep>, f64, f64)
```

**Step 3: Update `greedy_objective` signature**

```rust
fn greedy_objective(actor: &Actor, ...)
```

**Step 4: Update `evaluate` signature**

```rust
pub fn evaluate(actor: &Actor, ...)
```

**Step 5: Update `collect_greedy_states` signature**

```rust
pub fn collect_greedy_states(actor: &Actor, ...)
```

**Step 6: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 7: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "refactor(rl): update rollouts and eval to use Actor/Critic"
```

---

### Task 4: Update `train_reinforce`

**Files:**
- Modify: `builder/src/rl/train.rs`

**Step 1: Update `TrainResult`**

```rust
pub struct TrainResult {
    pub actor: Actor,
    pub final_train_reward: f64,
    pub train_pnl: f64,
    pub val_pnl: f64,
    pub test_pnl: f64,
    pub episodes: usize,
    pub best_episode: usize,
    pub metrics: Vec<EpisodeMetric>,
}
```

**Step 2: Update `train_reinforce`**

- Create `Actor` instead of `MLP`
- Create `Grads::zero(&actor.net)` and `AdamState::zero(&actor.net)`
- Update `rollout` call
- Update `greedy_objective` call
- Update `evaluate` calls
- Return `actor` in `TrainResult`

**Step 3: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 4: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "refactor(rl): update REINFORCE training for Actor-only"
```

---

### Task 5: Update `train_ppo`

**Files:**
- Modify: `builder/src/rl/train.rs`

**Step 1: Update `train_ppo`**

- Create both `Actor` and `Critic`
- Create separate `Grads` and `AdamState` for each
- Update `rollout_ppo` call to pass both
- Split PPO gradient accumulation:
  - `actor.accumulate_grad_ppo(...)` for policy
  - `critic.accumulate_grad(...)` for value
- Apply Adam separately to actor and critic
- Update `greedy_objective` call (uses actor)
- Update `evaluate` calls (uses actor)
- Return `actor` in `TrainResult`

**Step 2: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 3: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "refactor(rl): update PPO training for separate Actor and Critic"
```

---

### Task 6: Update `weights_to_bytes`

**Files:**
- Modify: `builder/src/rl/train.rs`

**Step 1: Update signature**

```rust
pub fn weights_to_bytes(actor: &Actor) -> Result<Vec<u8>, String>
```

**Step 2: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 3: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "refactor(rl): update weights_to_bytes for Actor"
```

---

### Task 7: Update `distill.rs`

**Files:**
- Modify: `builder/src/rl/distill.rs`

**Step 1: Update imports and signatures**

- Change `MLP` to `Actor`
- Update `dominant_logit`, `feature_importance`, `distil`, `net_to_rust` signatures

**Step 2: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 3: Commit**

```bash
git add builder/src/rl/distill.rs
git commit -m "refactor(rl): update distill for Actor"
```

---

### Task 8: Update `main.rs`

**Files:**
- Modify: `builder/src/main.rs`

**Step 1: Update imports**

- Change `MLP` to `Actor` in imports
- Update `collect_greedy_states`, `weights_to_bytes` usage

**Step 2: Update `TrainResult` usage**

- `result.net` → `result.actor`

**Step 3: Verify compilation**

Run: `cargo check --manifest-path builder/Cargo.toml`

**Step 4: Commit**

```bash
git add builder/src/main.rs
git commit -m "refactor(rl): update main.rs for Actor/Critic separation"
```

---

### Task 9: Update Tests

**Files:**
- Modify: `builder/src/rl/train.rs` (test module)

**Step 1: Rewrite tests**

- Test `MLP::forward` (pure feedforward)
- Test `Actor::forward_full`, `sample_action`, `greedy_action`
- Test `Critic::forward_value`
- Test `Actor::accumulate_grad` (REINFORCE)
- Test `Actor::accumulate_grad_ppo` + `Critic::accumulate_grad` (PPO)
- Test Adam update on both
- Test regularization on both

**Step 2: Run tests**

Run: `cargo test --manifest-path builder/Cargo.toml`
Expected: all tests pass

**Step 3: Commit**

```bash
git add builder/src/rl/train.rs
git commit -m "test(rl): update tests for Actor/Critic architecture"
```

---

### Task 10: Final Verification and Push

**Step 1: Full test suite**

Run: `cargo test --manifest-path builder/Cargo.toml`
Expected: all tests pass

**Step 2: Check all warnings**

Run: `cargo check --manifest-path builder/Cargo.toml`
Expected: clean (no errors, minimal warnings)

**Step 3: Push branch**

```bash
git push
```
