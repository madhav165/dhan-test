use ndarray::Array2;
use rand::Rng;

pub type Action = u8; // 0 = hold, 1 = buy, 2 = sell/short

// 2-layer MLP: input → tanh → tanh → softmax over 3 actions
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
    // baseline: subtract mean to reduce variance
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    returns.iter().map(|r| r - mean).collect()
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
            reward -= charges.cost(price, price) / price.max(1e-8);
        }
        (0, 2) if config.allow_short => {
            *position = -1; *entry_price = price; *holding = 0; *trades += 1;
            reward -= charges.cost(price, price) / price.max(1e-8);
        }
        (1, 0) | (1, 2) => {
            reward += (price - *entry_price) / entry_price.max(1e-8)
                - charges.cost(*entry_price, price) / entry_price.max(1e-8);
            *position = 0; *holding = 0;
            if action == 2 && config.allow_short {
                *position = -1; *entry_price = price; *trades += 1;
                reward -= charges.cost(price, price) / price.max(1e-8);
            }
        }
        (-1, 0) | (-1, 1) => {
            reward += (*entry_price - price) / entry_price.max(1e-8)
                - charges.cost(price, *entry_price) / entry_price.max(1e-8);
            *position = 0; *holding = 0;
            if action == 1 {
                *position = 1; *entry_price = price; *trades += 1;
                reward -= charges.cost(price, price) / price.max(1e-8);
            }
        }
        _ => { *holding += 1; }
    }

    if *position != 0 { *holding += 1; }

    if let (Some(max_d), Some(w)) = (config.max_holding_days, config.penalty_holding_days) {
        let d = *holding as f64 / 26.0;
        if d > max_d as f64 { reward -= w * (d - max_d as f64); }
    }
    if let (Some(max_t), Some(w)) = (config.max_trades_per_month, config.penalty_trades_per_month) {
        let rate = *trades as f64 / 20.0;
        if rate > max_t as f64 { reward -= w * (rate - max_t as f64); }
    }
    reward
}

struct Step { state: Vec<f64>, action: Action, ret: f64 }

fn rollout(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
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

    for t in 0..n {
        let state: Vec<f64> = states.row(t).to_vec();
        let (action, _) = net.sample_action(&state, rng, config.allow_short);
        let price = closes[state_offset + t];
        let r = step_reward(action, &mut position, &mut entry_price, &mut holding, &mut trades, price, config, charges);
        steps.push(Step { state, action, ret: 0.0 });
        rewards.push(r);
    }

    let returns = compute_returns(&rewards, config.gamma);
    let episode_return = rewards.iter().sum::<f64>();
    for (s, r) in steps.iter_mut().zip(returns) { s.ret = r; }
    (steps, episode_return)
}

pub struct TrainResult {
    pub net: MLP,
    pub final_train_reward: f64,
    pub val_pnl: f64,
    pub episodes: usize,
}

pub fn train(
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> TrainResult {
    let n = states.nrows();
    let train_end = ((n as f64 * 0.8) as usize).max(1);
    let train_states = states.slice(ndarray::s![..train_end, ..]).to_owned();
    let val_states = states.slice(ndarray::s![train_end.., ..]).to_owned();

    let input_size = states.ncols();
    let hidden_size = 32; // smaller = faster, still expressive enough
    let mut net = MLP::new(input_size, hidden_size);
    let mut m = AdamState::zero(&net);
    let mut v = AdamState::zero(&net);
    let mut rng = rand::rng();
    let mut final_train_reward = 0.0f64;

    for ep in 0..config.max_episodes {
        let (steps, episode_return) = rollout(&net, &train_states, closes, state_offset, config, charges, &mut rng);
        final_train_reward = episode_return;

        let mut grads = Grads::zero(&net);
        for step in &steps {
            net.accumulate_grad(&step.state, step.action, step.ret, &mut grads);
        }
        // normalise grads by trajectory length
        let n_steps = steps.len() as f64;
        macro_rules! norm { ($v:expr) => { for x in $v.iter_mut() { *x /= n_steps; } }; }
        norm!(grads.w1); norm!(grads.b1); norm!(grads.w2); norm!(grads.b2);
        norm!(grads.w_out); norm!(grads.b_out);

        net.apply_adam(&grads, &mut m, &mut v, ep + 1, config.lr);

        if ep % 50 == 0 {
            eprintln!("rl train: episode {}/{} return={:.4}", ep, config.max_episodes, final_train_reward);
        }
    }

    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&net, &val_states, closes, state_offset + train_end, config.allow_short, charges)
    } else { 0.0 };

    eprintln!("rl train: done. val_pnl={:.4}", val_pnl);
    TrainResult { net, final_train_reward, val_pnl, episodes: config.max_episodes }
}

pub fn evaluate(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
    allow_short: bool,
    charges: &BrokerCharges,
) -> f64 {
    let n = states.nrows();
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut total_pnl = 0.0f64;

    for t in 0..n {
        let state = states.row(t).to_vec();
        let action = net.greedy_action(&state, allow_short);
        let price = closes[state_offset + t];

        match (position, action) {
            (0, 1) => { position = 1; entry_price = price; }
            (0, 2) if allow_short => { position = -1; entry_price = price; }
            (1, 0) | (1, 2) => {
                total_pnl += (price - entry_price) / entry_price.max(1e-8)
                    - charges.cost(entry_price, price) / entry_price.max(1e-8);
                position = 0;
                if action == 2 && allow_short { position = -1; entry_price = price; }
            }
            (-1, 0) | (-1, 1) => {
                total_pnl += (entry_price - price) / entry_price.max(1e-8)
                    - charges.cost(price, entry_price) / entry_price.max(1e-8);
                position = 0;
                if action == 1 { position = 1; entry_price = price; }
            }
            _ => {}
        }
    }
    if position != 0 && n > 0 {
        let last = closes[state_offset + n - 1];
        if position == 1 {
            total_pnl += (last - entry_price) / entry_price.max(1e-8)
                - charges.cost(entry_price, last) / entry_price.max(1e-8);
        } else {
            total_pnl += (entry_price - last) / entry_price.max(1e-8)
                - charges.cost(last, entry_price) / entry_price.max(1e-8);
        }
    }
    total_pnl
}

pub fn weights_to_bytes(net: &MLP) -> Vec<u8> {
    net.params().iter().flat_map(|&v| v.to_le_bytes()).collect()
}
