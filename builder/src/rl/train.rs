use ndarray::Array2;
use rand::Rng;

pub type Action = u8; // 0 = hold, 1 = buy, 2 = sell/short

// 2-layer MLP: input → tanh → tanh → softmax over 3 actions
#[derive(Clone)]
pub struct MLP {
    pub w1: Vec<f64>,   // hidden × input
    pub b1: Vec<f64>,   // hidden
    pub w2: Vec<f64>,   // hidden × hidden
    pub b2: Vec<f64>,   // hidden
    pub w_out: Vec<f64>, // 3 × hidden
    pub b_out: Vec<f64>, // 3
    pub input_size: usize,
    pub hidden_size: usize,
}

impl MLP {
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        use rand_distr::{Normal, Distribution};
        let mut rng = rand::rng();
        let scale = (2.0 / input_size as f64).sqrt();
        let normal = Normal::new(0.0, scale).unwrap();
        let mut init = |n: usize| -> Vec<f64> { (0..n).map(|_| normal.sample(&mut rng)).collect() };
        Self {
            w1: init(hidden_size * input_size),
            b1: vec![0.0; hidden_size],
            w2: init(hidden_size * hidden_size),
            b2: vec![0.0; hidden_size],
            w_out: init(3 * hidden_size),
            b_out: vec![0.0; 3],
            input_size,
            hidden_size,
        }
    }

    fn matmul(w: &[f64], x: &[f64], rows: usize, cols: usize) -> Vec<f64> {
        (0..rows).map(|i| (0..cols).map(|j| w[i * cols + j] * x[j]).sum::<f64>()).collect()
    }

    /// Forward pass. Returns (h1, h2, probs) — intermediates needed for backprop.
    pub fn forward_full(&self, x: &[f64]) -> (Vec<f64>, Vec<f64>, [f64; 3]) {
        let pre_h1: Vec<f64> = Self::matmul(&self.w1, x, self.hidden_size, self.input_size)
            .iter().zip(&self.b1).map(|(v, b)| v + b).collect();
        let h1: Vec<f64> = pre_h1.iter().map(|v| v.tanh()).collect();

        let pre_h2: Vec<f64> = Self::matmul(&self.w2, &h1, self.hidden_size, self.hidden_size)
            .iter().zip(&self.b2).map(|(v, b)| v + b).collect();
        let h2: Vec<f64> = pre_h2.iter().map(|v| v.tanh()).collect();

        let logits: Vec<f64> = Self::matmul(&self.w_out, &h2, 3, self.hidden_size)
            .iter().zip(&self.b_out).map(|(v, b)| v + b).collect();

        let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp: Vec<f64> = logits.iter().map(|v| (v - max).exp()).collect();
        let sum: f64 = exp.iter().sum();
        let probs = [exp[0] / sum, exp[1] / sum, exp[2] / sum];

        (h1, h2, probs)
    }

    pub fn probs(&self, x: &[f64], mask_sell: bool) -> [f64; 3] {
        let (_, _, mut p) = self.forward_full(x);
        if mask_sell {
            // renormalise without sell
            let s = p[0] + p[1];
            p[0] /= s; p[1] /= s; p[2] = 0.0;
        }
        p
    }

    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (Action, f64) {
        let p = self.probs(x, !allow_short);
        let r: f64 = rng.random();
        let action = if r < p[0] { 0u8 } else if r < p[0] + p[1] { 1 } else { 2 };
        (action, p[action as usize].max(1e-12).ln())
    }

    pub fn greedy_action(&self, x: &[f64], allow_short: bool) -> Action {
        let p = self.probs(x, !allow_short);
        if p[0] >= p[1] && p[0] >= p[2] { 0 } else if p[1] >= p[2] { 1 } else { 2 }
    }

    /// REINFORCE gradient: accumulate -G_t * ∇log π(a_t|s_t) via backprop.
    /// Caller accumulates over the trajectory then calls apply_adam.
    pub fn accumulate_grad(
        &self,
        x: &[f64],
        action: Action,
        advantage: f64,
        grads: &mut Grads,
    ) {
        let (h1, h2, probs) = self.forward_full(x);

        // dL/d_logits = -(advantage) * (1_{a} - p)  [REINFORCE, negated because we minimise -return]
        let mut d_logits = [0.0f64; 3];
        for k in 0..3 {
            let indicator = if k == action as usize { 1.0 } else { 0.0 };
            d_logits[k] = -advantage * (indicator - probs[k]);
        }

        // w_out gradient: d_logits (3) × h2^T (hidden) → (3 × hidden)
        for i in 0..3 {
            for j in 0..self.hidden_size {
                grads.w_out[i * self.hidden_size + j] += d_logits[i] * h2[j];
            }
            grads.b_out[i] += d_logits[i];
        }

        // backprop through h2: d_h2 = w_out^T × d_logits
        let mut d_h2 = vec![0.0f64; self.hidden_size];
        for j in 0..self.hidden_size {
            for i in 0..3 {
                d_h2[j] += self.w_out[i * self.hidden_size + j] * d_logits[i];
            }
        }

        // backprop through tanh at h2: d_pre_h2 = d_h2 * (1 - h2²)
        let d_pre_h2: Vec<f64> = h2.iter().zip(&d_h2).map(|(h, &g)| g * (1.0 - h * h)).collect();

        // w2, b2 gradients
        for i in 0..self.hidden_size {
            for j in 0..self.hidden_size {
                grads.w2[i * self.hidden_size + j] += d_pre_h2[i] * h1[j];
            }
            grads.b2[i] += d_pre_h2[i];
        }

        // backprop through h1
        let mut d_h1 = vec![0.0f64; self.hidden_size];
        for j in 0..self.hidden_size {
            for i in 0..self.hidden_size {
                d_h1[j] += self.w2[i * self.hidden_size + j] * d_pre_h2[i];
            }
        }

        let d_pre_h1: Vec<f64> = h1.iter().zip(&d_h1).map(|(h, &g)| g * (1.0 - h * h)).collect();

        // w1, b1 gradients
        for i in 0..self.hidden_size {
            for j in 0..self.input_size {
                grads.w1[i * self.input_size + j] += d_pre_h1[i] * x[j];
            }
            grads.b1[i] += d_pre_h1[i];
        }
    }

    pub fn apply_adam(&mut self, grads: &Grads, m: &mut AdamState, v: &mut AdamState, t: usize, lr: f64) {
        let (beta1, beta2, eps) = (0.9f64, 0.999f64, 1e-8f64);
        let bc1 = 1.0 - beta1.powi(t as i32);
        let bc2 = 1.0 - beta2.powi(t as i32);

        macro_rules! update {
            ($param:expr, $grad:expr, $m:expr, $v:expr) => {
                for i in 0..$param.len() {
                    $m[i] = beta1 * $m[i] + (1.0 - beta1) * $grad[i];
                    $v[i] = beta2 * $v[i] + (1.0 - beta2) * $grad[i].powi(2);
                    $param[i] -= lr * ($m[i] / bc1) / (($v[i] / bc2).sqrt() + eps);
                }
            };
        }
        update!(self.w1, grads.w1, m.w1, v.w1);
        update!(self.b1, grads.b1, m.b1, v.b1);
        update!(self.w2, grads.w2, m.w2, v.w2);
        update!(self.b2, grads.b2, m.b2, v.b2);
        update!(self.w_out, grads.w_out, m.w_out, v.w_out);
        update!(self.b_out, grads.b_out, m.b_out, v.b_out);
    }

    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        p.extend_from_slice(&self.w1); p.extend_from_slice(&self.b1);
        p.extend_from_slice(&self.w2); p.extend_from_slice(&self.b2);
        p.extend_from_slice(&self.w_out); p.extend_from_slice(&self.b_out);
        p
    }
}

pub struct Grads {
    pub w1: Vec<f64>, pub b1: Vec<f64>,
    pub w2: Vec<f64>, pub b2: Vec<f64>,
    pub w_out: Vec<f64>, pub b_out: Vec<f64>,
}

impl Grads {
    fn zero(net: &MLP) -> Self {
        Self {
            w1: vec![0.0; net.w1.len()], b1: vec![0.0; net.b1.len()],
            w2: vec![0.0; net.w2.len()], b2: vec![0.0; net.b2.len()],
            w_out: vec![0.0; net.w_out.len()], b_out: vec![0.0; net.b_out.len()],
        }
    }

    fn clip_global_norm(&mut self, max_norm: f64) {
        let sum_sq = self.w1.iter().chain(&self.b1)
            .chain(&self.w2).chain(&self.b2)
            .chain(&self.w_out).chain(&self.b_out)
            .map(|v| v * v)
            .sum::<f64>();
        let norm = sum_sq.sqrt();
        if norm <= max_norm || norm < 1e-12 {
            return;
        }
        let scale = max_norm / norm;
        for v in self.w1.iter_mut().chain(&mut self.b1)
            .chain(&mut self.w2).chain(&mut self.b2)
            .chain(&mut self.w_out).chain(&mut self.b_out) {
            *v *= scale;
        }
    }
}

pub struct AdamState {
    pub w1: Vec<f64>, pub b1: Vec<f64>,
    pub w2: Vec<f64>, pub b2: Vec<f64>,
    pub w_out: Vec<f64>, pub b_out: Vec<f64>,
}

impl AdamState {
    fn zero(net: &MLP) -> Self {
        Self {
            w1: vec![0.0; net.w1.len()], b1: vec![0.0; net.b1.len()],
            w2: vec![0.0; net.w2.len()], b2: vec![0.0; net.b2.len()],
            w_out: vec![0.0; net.w_out.len()], b_out: vec![0.0; net.b_out.len()],
        }
    }
}

#[derive(Clone)]
pub struct BrokerCharges {
    pub brokerage_flat: f64,
    pub brokerage_pct: f64,
    pub stt_buy_pct: f64,
    pub stt_sell_pct: f64,
    pub exchange_pct: f64,
    pub sebi_pct: f64,
    pub stamp_buy_pct: f64,
    pub gst_pct: f64,
}

impl BrokerCharges {
    pub fn cost(&self, buy_price: f64, sell_price: f64) -> f64 {
        let brokerage = (self.brokerage_pct * buy_price).min(self.brokerage_flat)
            + (self.brokerage_pct * sell_price).min(self.brokerage_flat);
        let stt = self.stt_buy_pct * buy_price + self.stt_sell_pct * sell_price;
        let exchange = self.exchange_pct * (buy_price + sell_price);
        let sebi = self.sebi_pct * (buy_price + sell_price);
        let stamp = self.stamp_buy_pct * buy_price;
        let gst = self.gst_pct * (brokerage + exchange + sebi);
        brokerage + stt + exchange + sebi + stamp + gst
    }
}

pub struct TrainConfig {
    pub max_episodes: usize,
    pub episode_steps: usize,
    pub validation_interval: usize,
    pub early_stopping_patience: usize,
    pub min_delta: f64,
    pub grad_clip_norm: f64,
    pub lr: f64,
    pub gamma: f64,
    pub allow_short: bool,
    pub reward_type: String,
    pub penalty_holding_days: Option<f64>,
    pub max_holding_days: Option<usize>,
    pub penalty_trades_per_month: Option<f64>,
    pub max_trades_per_month: Option<usize>,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            max_episodes: 500,
            episode_steps: 200,
            validation_interval: 10,
            early_stopping_patience: 50,
            min_delta: 1e-6,
            grad_clip_norm: 1.0,
            lr: 3e-4,
            gamma: 0.99,
            allow_short: false,
            reward_type: "pnl".into(),
            penalty_holding_days: None,
            max_holding_days: None,
            penalty_trades_per_month: None,
            max_trades_per_month: None,
        }
    }
}

fn compute_returns(rewards: &[f64], gamma: f64) -> Vec<f64> {
    let mut returns = vec![0.0; rewards.len()];
    let mut g = 0.0;
    for i in (0..rewards.len()).rev() {
        g = rewards[i] + gamma * g;
        returns[i] = g;
    }
    // baseline: standardise advantages to reduce variance and scale sensitivity
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    let std = var.sqrt().max(1e-8);
    returns.iter().map(|r| (r - mean) / std).collect()
}

pub fn split_points(n: usize) -> (usize, usize) {
    if n < 3 {
        return (n.max(1), n);
    }
    let train_end = ((n as f64 * 0.70).floor() as usize).clamp(1, n - 2);
    let val_end = ((n as f64 * 0.85).floor() as usize).clamp(train_end + 1, n - 1);
    (train_end, val_end)
}

fn step_reward(
    action: Action,
    position: &mut i8,
    entry_price: &mut f64,
    holding: &mut usize,
    trades: &mut usize,
    price: f64,
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> f64 {
    let mut reward = 0.0;

    match (*position, action) {
        (0, 1) => {
            *position = 1; *entry_price = price; *holding = 0; *trades += 1;
        }
        (0, 2) if config.allow_short => {
            *position = -1; *entry_price = price; *holding = 0; *trades += 1;
        }
        (1, 0) | (1, 2) => {
            reward += (price - *entry_price) - charges.cost(*entry_price, price);
            *trades += 1;
            *position = 0; *holding = 0;
            if action == 2 && config.allow_short {
                *position = -1; *entry_price = price; *trades += 1;
            }
        }
        (-1, 0) | (-1, 1) => {
            reward += (*entry_price - price) - charges.cost(price, *entry_price);
            *trades += 1;
            *position = 0; *holding = 0;
            if action == 1 {
                *position = 1; *entry_price = price; *trades += 1;
            }
        }
        _ => {}
    }

    if *position != 0 { *holding += 1; }

    if let (Some(max_d), Some(w)) = (config.max_holding_days, config.penalty_holding_days) {
        let d = *holding as f64;
        if d > max_d as f64 { reward -= w * (d - max_d as f64); }
    }
    if let (Some(max_t), Some(w)) = (config.max_trades_per_month, config.penalty_trades_per_month) {
        let rate = *trades as f64 / 20.0;
        if rate > max_t as f64 { reward -= w * (rate - max_t as f64); }
    }
    reward
}

struct Step { state: Vec<f64>, action: Action, ret: f64 }

fn position_features(position: i8, holding: usize, entry_price: f64, price: f64) -> [f64; 3] {
    let unrealized = match position {
        1 => price - entry_price,
        -1 => entry_price - price,
        _ => 0.0,
    };
    // Scale to roughly unit variance so they don't swamp normalized base features
    [
        position as f64 * 0.5,
        (holding as f64 / 20.0).clamp(-5.0, 5.0),
        (unrealized / entry_price.max(1e-8)).clamp(-5.0, 5.0),
    ]
}

fn decision_state(base_state: &[f64], position: i8, holding: usize, entry_price: f64, price: f64) -> Vec<f64> {
    let mut state = Vec::with_capacity(base_state.len() + 3);
    state.extend_from_slice(base_state);
    state.extend_from_slice(&position_features(position, holding, entry_price, price));
    state
}

fn close_position_reward(position: i8, entry_price: f64, price: f64, charges: &BrokerCharges) -> f64 {
    match position {
        1 => (price - entry_price) - charges.cost(entry_price, price),
        -1 => (entry_price - price) - charges.cost(price, entry_price),
        _ => 0.0,
    }
}

/// Walk the data greedily and compute the training objective metric (PnL, Sharpe, etc.).
/// Used for validation during training so early stopping aligns with the reward_type.
fn greedy_objective(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> f64 {
    let n = states.nrows();
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut rewards = vec![];

    for t in 0..n {
        let base_state = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let state = decision_state(&base_state, position, holding, entry_price, price);
        let action = net.greedy_action(&state, config.allow_short);

        let r = step_reward(action, &mut position, &mut entry_price, &mut holding, &mut trades, price, config, charges);
        rewards.push(r);
    }

    match config.reward_type.as_str() {
        "sharpe" => {
            let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
            let var = rewards.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rewards.len() as f64;
            let std = var.sqrt();
            if std < 1e-12 { 0.0 } else { mean / std }
        }
        "min_drawdown" => {
            let mut equity = 0.0f64;
            let mut peak = 0.0f64;
            let mut max_dd = 0.0f64;
            for r in &rewards {
                equity += r;
                if equity > peak { peak = equity; }
                max_dd = max_dd.max(peak - equity);
            }
            -max_dd
        }
        _ => rewards.iter().sum::<f64>(),
    }
}

/// Walk the data greedily and collect the actual decision states (including position features).
/// Used for feature importance and distillation so they reflect real agent behaviour.
pub fn collect_greedy_states(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    allow_short: bool,
) -> Array2<f64> {
    let n = states.nrows();
    let base_dim = states.ncols();
    let state_dim = base_dim + 3;
    let mut out = Array2::zeros((n, state_dim));
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;

    for t in 0..n {
        let base = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let pf = position_features(position, holding, entry_price, price);

        for (j, &v) in base.iter().enumerate() {
            out[[t, j]] = v;
        }
        for (j, &v) in pf.iter().enumerate() {
            out[[t, base_dim + j]] = v;
        }

        let action = net.greedy_action(
            &decision_state(&base, position, holding, entry_price, price),
            allow_short,
        );

        match (position, action) {
            (0, 1) => { position = 1; entry_price = price; holding = 0; }
            (0, 2) if allow_short => { position = -1; entry_price = price; holding = 0; }
            (1, 0) | (1, 2) => {
                position = 0; holding = 0;
                if action == 2 && allow_short { position = -1; entry_price = price; }
            }
            (-1, 0) | (-1, 1) => {
                position = 0; holding = 0;
                if action == 1 { position = 1; entry_price = price; }
            }
            _ => {}
        }
        if position != 0 { holding += 1; }
    }
    out
}

fn rollout(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
    rng: &mut impl Rng,
) -> (Vec<Step>, f64) {
    let n = states.nrows().min(config.episode_steps);
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut steps = vec![];
    let mut rewards = vec![];

    let start = if states.nrows() > n {
        rng.random_range(0..=(states.nrows() - n))
    } else {
        0
    };

    for t in 0..n {
        let row_idx = start + t;
        let price = closes[candle_indices[row_idx]];
        let state = decision_state(&states.row(row_idx).to_vec(), position, holding, entry_price, price);
        let (action, _) = net.sample_action(&state, rng, config.allow_short);
        let r = step_reward(action, &mut position, &mut entry_price, &mut holding, &mut trades, price, config, charges);
        steps.push(Step { state, action, ret: 0.0 });
        rewards.push(r);
    }

    if position != 0 && n > 0 {
        let last_idx = start + n - 1;
        let last_price = closes[candle_indices[last_idx]];
        if let Some(last_reward) = rewards.last_mut() {
            *last_reward += close_position_reward(position, entry_price, last_price, charges);
        }
    }

    let (returns, episode_return) = objective_returns(&rewards, config);
    for (s, r) in steps.iter_mut().zip(returns) { s.ret = r; }
    (steps, episode_return)
}

fn objective_returns(rewards: &[f64], config: &TrainConfig) -> (Vec<f64>, f64) {
    if rewards.is_empty() {
        return (vec![], 0.0);
    }

    match config.reward_type.as_str() {
        "sharpe" => {
            let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
            let var = rewards.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rewards.len() as f64;
            let std = var.sqrt();
            let objective = if std < 1e-12 { 0.0 } else { mean / std };
            if std < 1e-12 {
                return (compute_returns(rewards, config.gamma), objective);
            }
            let shaped: Vec<f64> = rewards.iter()
                .map(|r| *r - 0.5 * (r - mean).powi(2) / std)
                .collect();
            (compute_returns(&shaped, config.gamma), objective)
        }
        "min_drawdown" => {
            let mut equity = 0.0f64;
            let mut peak = 0.0f64;
            let mut max_dd = 0.0f64;
            let mut shaped = Vec::with_capacity(rewards.len());
            for r in rewards {
                equity += r;
                if equity > peak { peak = equity; }
                let dd = peak - equity;
                let dd_increase = (dd - max_dd).max(0.0);
                max_dd = max_dd.max(dd);
                shaped.push(*r - dd_increase);
            }
            let objective = -max_dd;
            (compute_returns(&shaped, config.gamma), objective)
        }
        _ => {
            let episode_return = rewards.iter().sum::<f64>();
            (compute_returns(rewards, config.gamma), episode_return)
        }
    }
}

#[derive(Clone, Debug)]
pub struct EpisodeMetric {
    pub episode: usize,
    pub train_reward: f64,
    pub val_metric: Option<f64>,
}

pub struct TrainResult {
    pub net: MLP,
    pub final_train_reward: f64,
    pub train_pnl: f64,
    pub val_pnl: f64,
    pub test_pnl: f64,
    pub episodes: usize,
    pub best_episode: usize,
    pub metrics: Vec<EpisodeMetric>,
}

pub fn train(
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> TrainResult {
    let n = states.nrows();
    assert_eq!(n, candle_indices.len(), "state rows and candle indices must match");
    let (train_end, val_end) = split_points(n);
    let train_states = states.slice(ndarray::s![..train_end, ..]).to_owned();
    let val_states = states.slice(ndarray::s![train_end..val_end, ..]).to_owned();
    let test_states = states.slice(ndarray::s![val_end.., ..]).to_owned();
    let train_indices = &candle_indices[..train_end];
    let val_indices = &candle_indices[train_end..val_end];
    let test_indices = &candle_indices[val_end..];

    let input_size = states.ncols() + 3;
    let hidden_size = 32; // smaller = faster, still expressive enough
    let mut net = MLP::new(input_size, hidden_size);
    let mut m = AdamState::zero(&net);
    let mut v = AdamState::zero(&net);
    let mut rng = rand::rng();
    let mut final_train_reward = 0.0f64;
    let mut reward_ema: Option<f64> = None;
    let mut best_net = net.clone();
    let mut best_val_metric = f64::NEG_INFINITY;
    let mut best_episode = 0usize;
    let mut episodes_since_best = 0usize;
    let mut episodes_run = 0usize;
    let mut metrics: Vec<EpisodeMetric> = vec![];
    let validation_interval = config.validation_interval.max(1);

    for ep in 0..config.max_episodes {
        episodes_run = ep + 1;
        let (steps, episode_return) = rollout(&net, &train_states, closes, train_indices, config, charges, &mut rng);
        reward_ema = Some(match reward_ema {
            Some(prev) => 0.95 * prev + 0.05 * episode_return,
            None => episode_return,
        });
        final_train_reward = reward_ema.unwrap_or(episode_return);

        let mut grads = Grads::zero(&net);
        for step in &steps {
            net.accumulate_grad(&step.state, step.action, step.ret, &mut grads);
        }
        // normalise grads by trajectory length
        let n_steps = steps.len() as f64;
        macro_rules! norm { ($v:expr) => { for x in $v.iter_mut() { *x /= n_steps; } }; }
        norm!(grads.w1); norm!(grads.b1); norm!(grads.w2); norm!(grads.b2);
        norm!(grads.w_out); norm!(grads.b_out);
        grads.clip_global_norm(config.grad_clip_norm);

        net.apply_adam(&grads, &mut m, &mut v, ep + 1, config.lr);

        eprintln!("rl train: episode {}/{} return={:.4}", ep, config.max_episodes, final_train_reward);

        let mut val_metric_opt: Option<f64> = None;
        if val_states.nrows() > 0 && ((ep + 1) % validation_interval == 0 || ep + 1 == config.max_episodes) {
            let val_metric = greedy_objective(&net, &val_states, closes, val_indices, config, charges);
            val_metric_opt = Some(val_metric);
            if val_metric > best_val_metric + config.min_delta {
                best_val_metric = val_metric;
                best_net = net.clone();
                best_episode = ep + 1;
                episodes_since_best = 0;
            } else {
                episodes_since_best += validation_interval;
                if episodes_since_best >= config.early_stopping_patience {
                    eprintln!("rl train: early stopping at episode {} best_episode={} best_val_metric={:.4}", ep + 1, best_episode, best_val_metric);
                    break;
                }
            }
        }

        metrics.push(EpisodeMetric {
            episode: ep + 1,
            train_reward: final_train_reward,
            val_metric: val_metric_opt,
        });
    }

    if best_episode > 0 {
        net = best_net;
    }

    let train_pnl = evaluate(&net, &train_states, closes, train_indices, config.allow_short, charges);

    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&net, &val_states, closes, val_indices, config.allow_short, charges)
    } else { 0.0 };
    let test_pnl = if test_states.nrows() > 0 {
        evaluate(&net, &test_states, closes, test_indices, config.allow_short, charges)
    } else { 0.0 };
    eprintln!("rl train: done. train_pnl={:.4} val_pnl={:.4} test_pnl={:.4}", train_pnl, val_pnl, test_pnl);
    TrainResult { net, final_train_reward, train_pnl, val_pnl, test_pnl, episodes: episodes_run, best_episode, metrics }
}

pub fn evaluate(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    allow_short: bool,
    charges: &BrokerCharges,
) -> f64 {
    let n = states.nrows();
    assert_eq!(n, candle_indices.len(), "state rows and candle indices must match");
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut total_pnl = 0.0f64;

    for t in 0..n {
        let state = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let state = decision_state(&state, position, holding, entry_price, price);
        let action = net.greedy_action(&state, allow_short);

        match (position, action) {
            (0, 1) => { position = 1; entry_price = price; holding = 0; }
            (0, 2) if allow_short => { position = -1; entry_price = price; holding = 0; }
            (1, 0) | (1, 2) => {
                total_pnl += close_position_reward(position, entry_price, price, charges);
                position = 0; holding = 0;
                if action == 2 && allow_short { position = -1; entry_price = price; }
            }
            (-1, 0) | (-1, 1) => {
                total_pnl += close_position_reward(position, entry_price, price, charges);
                position = 0; holding = 0;
                if action == 1 { position = 1; entry_price = price; }
            }
            _ => {}
        }
        if position != 0 { holding += 1; }
    }
    if position != 0 && n > 0 {
        let last = closes[candle_indices[n - 1]];
        total_pnl += close_position_reward(position, entry_price, last, charges);
    }
    total_pnl
}

pub fn weights_to_bytes(net: &MLP) -> Result<Vec<u8>, String> {
    let params = net.params();
    if params.iter().any(|v| !v.is_finite()) {
        return Err("training diverged: network weights contain NaN/Inf".to_string());
    }
    Ok(params.iter().flat_map(|&v| v.to_le_bytes()).collect())
}
