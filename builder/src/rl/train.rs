use ndarray::Array2;
use rand::Rng;

/// 0 = hold, 1 = buy, 2 = sell/short
pub type Action = u8;

pub struct MLP {
    pub w1: Vec<f64>,
    pub b1: Vec<f64>,
    pub w2: Vec<f64>,
    pub b2: Vec<f64>,
    pub w_out: Vec<f64>,  // 3 × hidden_size  (logits for hold/buy/sell)
    pub b_out: Vec<f64>,  // 3
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

    fn matmul_bias(w: &[f64], b: &[f64], x: &[f64], rows: usize, cols: usize) -> Vec<f64> {
        (0..rows).map(|i| b[i] + (0..cols).map(|j| w[i * cols + j] * x[j]).sum::<f64>()).collect()
    }

    /// Returns logits for [hold, buy, sell].
    pub fn logits(&self, x: &[f64]) -> [f64; 3] {
        let h1: Vec<f64> = Self::matmul_bias(&self.w1, &self.b1, x, self.hidden_size, self.input_size)
            .iter().map(|&v| v.tanh()).collect();
        let h2: Vec<f64> = Self::matmul_bias(&self.w2, &self.b2, &h1, self.hidden_size, self.hidden_size)
            .iter().map(|&v| v.tanh()).collect();
        let out = Self::matmul_bias(&self.w_out, &self.b_out, &h2, 3, self.hidden_size);
        [out[0], out[1], out[2]]
    }

    /// Softmax over logits, optionally masking sell (action 2) when allow_short=false and no position.
    pub fn probs(&self, x: &[f64], mask_sell: bool) -> [f64; 3] {
        let mut lg = self.logits(x);
        if mask_sell { lg[2] = f64::NEG_INFINITY; }
        let max = lg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp: [f64; 3] = [
            (lg[0] - max).exp(),
            (lg[1] - max).exp(),
            (lg[2] - max).exp(),
        ];
        let sum = exp[0] + exp[1] + exp[2];
        [exp[0] / sum, exp[1] / sum, exp[2] / sum]
    }

    /// Sample action; returns (action, log_prob).
    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (Action, f64) {
        let p = self.probs(x, !allow_short);
        let r: f64 = rng.random();
        let action = if r < p[0] { 0 } else if r < p[0] + p[1] { 1 } else { 2 };
        let log_prob = p[action as usize].max(1e-12).ln();
        (action, log_prob)
    }

    /// Greedy action (argmax).
    pub fn greedy_action(&self, x: &[f64], allow_short: bool) -> Action {
        let p = self.probs(x, !allow_short);
        if p[0] >= p[1] && p[0] >= p[2] { 0 } else if p[1] >= p[2] { 1 } else { 2 }
    }

    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        p.extend_from_slice(&self.w1); p.extend_from_slice(&self.b1);
        p.extend_from_slice(&self.w2); p.extend_from_slice(&self.b2);
        p.extend_from_slice(&self.w_out); p.extend_from_slice(&self.b_out);
        p
    }

    pub fn load_params(&mut self, p: &[f64]) {
        let mut off = 0;
        let mut copy = |dst: &mut Vec<f64>| {
            let n = dst.len(); dst.copy_from_slice(&p[off..off+n]); off += n;
        };
        copy(&mut self.w1); copy(&mut self.b1);
        copy(&mut self.w2); copy(&mut self.b2);
        copy(&mut self.w_out); copy(&mut self.b_out);
    }

    pub fn param_count(&self) -> usize { self.params().len() }

    pub fn apply_adam(&mut self, grads: &[f64], m: &mut Vec<f64>, v: &mut Vec<f64>, t: usize, lr: f64) {
        let (beta1, beta2, eps) = (0.9f64, 0.999f64, 1e-8f64);
        let bc1 = 1.0 - beta1.powi(t as i32);
        let bc2 = 1.0 - beta2.powi(t as i32);
        let mut params = self.params();
        for i in 0..params.len() {
            m[i] = beta1 * m[i] + (1.0 - beta1) * grads[i];
            v[i] = beta2 * v[i] + (1.0 - beta2) * grads[i].powi(2);
            params[i] -= lr * (m[i] / bc1) / ((v[i] / bc2).sqrt() + eps);
        }
        self.load_params(&params);
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
    pub clip_eps: f64,
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
    actions: Vec<Action>,
    log_probs: Vec<f64>,
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

/// Simulate one episode, track positions properly, deduct transaction costs on open/close.
fn rollout(
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
    config: &TrainConfig,
    charges: &BrokerCharges,
    rng: &mut impl Rng,
) -> Trajectory {
    let n = states.nrows().min(config.episode_steps);
    // position: 0 = flat, 1 = long, -1 = short
    let mut position: i8 = 0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut rewards = vec![];
    let mut traj_states = vec![];
    let mut traj_actions = vec![];
    let mut traj_log_probs = vec![];

    for t in 0..n {
        let state: Vec<f64> = states.row(t).to_vec();
        let (action, lp) = net.sample_action(&state, rng, config.allow_short);
        let ci = state_offset + t;
        let price = closes[ci];

        let mut reward = 0.0f64;

        match (position, action) {
            (0, 1) => {
                // open long
                position = 1; entry_price = price; holding = 0; trades += 1;
                reward -= charges.cost(price, price) / price; // normalise to per-unit
            }
            (0, 2) if config.allow_short => {
                // open short
                position = -1; entry_price = price; holding = 0; trades += 1;
                reward -= charges.cost(price, price) / price;
            }
            (1, 2) | (1, 0) => {
                // close long
                let gross = (price - entry_price) / entry_price.max(1e-8);
                let cost = charges.cost(entry_price, price) / entry_price.max(1e-8);
                reward += gross - cost;
                position = 0; holding = 0;
                if action == 2 && config.allow_short {
                    // flip to short immediately
                    position = -1; entry_price = price; trades += 1;
                    reward -= charges.cost(price, price) / price;
                }
            }
            (-1, 1) | (-1, 0) => {
                // close short
                let gross = (entry_price - price) / entry_price.max(1e-8);
                let cost = charges.cost(price, entry_price) / entry_price.max(1e-8);
                reward += gross - cost;
                position = 0; holding = 0;
                if action == 1 {
                    // flip to long immediately
                    position = 1; entry_price = price; trades += 1;
                    reward -= charges.cost(price, price) / price;
                }
            }
            _ => { holding += 1; } // hold or redundant same-direction signal
        }

        // constraint penalties
        if position != 0 { holding += 1; }
        if let (Some(max_days), Some(w)) = (config.max_holding_days, config.penalty_holding_days) {
            let holding_days = holding as f64 / 26.0;
            if holding_days > max_days as f64 { reward -= w * (holding_days - max_days as f64); }
        }
        if let (Some(max_trades), Some(w)) = (config.max_trades_per_month, config.penalty_trades_per_month) {
            let monthly_rate = trades as f64 / 20.0;
            if monthly_rate > max_trades as f64 { reward -= w * (monthly_rate - max_trades as f64); }
        }

        traj_states.push(state);
        traj_actions.push(action);
        traj_log_probs.push(lp);
        rewards.push(reward);
    }

    // close any open position at end of episode
    // (no reward adjustment needed — already accounted in rollout)

    let returns = compute_returns(&rewards, config.gamma);
    Trajectory { states: traj_states, actions: traj_actions, log_probs: traj_log_probs, returns }
}

fn ppo_grad(net: &MLP, traj: &Trajectory, clip_eps: f64) -> Vec<f64> {
    let params = net.params();
    let h = 1e-4;

    (0..params.len()).map(|i| {
        let mut p_plus = params.clone();
        let mut p_minus = params.clone();
        p_plus[i] += h;
        p_minus[i] -= h;

        let mut net_p = MLP::new(net.input_size, net.hidden_size);
        net_p.load_params(&p_plus);
        let mut net_m = MLP::new(net.input_size, net.hidden_size);
        net_m.load_params(&p_minus);

        let loss = |n: &MLP| -> f64 {
            traj.states.iter().zip(&traj.actions).zip(&traj.log_probs).zip(&traj.returns)
                .map(|(((s, &a), &old_lp), &ret)| {
                    // log prob of the taken action under the new policy
                    let lg = n.logits(s);
                    let max = lg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let exp: [f64; 3] = [(lg[0]-max).exp(), (lg[1]-max).exp(), (lg[2]-max).exp()];
                    let sum = exp[0] + exp[1] + exp[2];
                    let lp = (exp[a as usize] / sum).max(1e-12).ln();
                    let ratio = (lp - old_lp).exp().clamp(1.0 - clip_eps, 1.0 + clip_eps);
                    -ratio * ret
                }).sum::<f64>()
        };

        (loss(&net_p) - loss(&net_m)) / (2.0 * h)
    }).collect()
}

/// Evaluate greedy policy on a state matrix; returns total net PnL per unit.
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
        let ci = state_offset + t;
        let price = closes[ci];

        match (position, action) {
            (0, 1) => { position = 1; entry_price = price; }
            (0, 2) if allow_short => { position = -1; entry_price = price; }
            (1, 2) | (1, 0) => {
                let gross = (price - entry_price) / entry_price.max(1e-8);
                total_pnl += gross - charges.cost(entry_price, price) / entry_price.max(1e-8);
                position = 0;
                if action == 2 && allow_short { position = -1; entry_price = price; }
            }
            (-1, 1) | (-1, 0) => {
                let gross = (entry_price - price) / entry_price.max(1e-8);
                total_pnl += gross - charges.cost(price, entry_price) / entry_price.max(1e-8);
                position = 0;
                if action == 1 { position = 1; entry_price = price; }
            }
            _ => {}
        }
    }
    // close open position at end
    if position != 0 {
        let last_price = closes[state_offset + n - 1];
        if position == 1 {
            total_pnl += (last_price - entry_price) / entry_price.max(1e-8)
                - charges.cost(entry_price, last_price) / entry_price.max(1e-8);
        } else {
            total_pnl += (entry_price - last_price) / entry_price.max(1e-8)
                - charges.cost(last_price, entry_price) / entry_price.max(1e-8);
        }
    }
    total_pnl
}

pub struct TrainResult {
    pub net: MLP,
    pub final_train_reward: f64,
    pub val_pnl: f64,
    pub episodes: usize,
}

/// Train on the first `train_frac` of states, validate on the remainder.
pub fn train(
    states: &Array2<f64>,
    closes: &[f64],
    state_offset: usize,
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> TrainResult {
    let n = states.nrows();
    let train_end = (n as f64 * 0.8) as usize;
    let train_states = states.slice(ndarray::s![..train_end, ..]).to_owned();
    let val_states = states.slice(ndarray::s![train_end.., ..]).to_owned();

    let input_size = states.ncols();
    let hidden_size = 64;
    let mut net = MLP::new(input_size, hidden_size);
    let param_count = net.param_count();
    let mut m = vec![0.0f64; param_count];
    let mut v = vec![0.0f64; param_count];
    let mut rng = rand::rng();
    let mut final_train_reward = 0.0f64;

    for ep in 0..config.max_episodes {
        let traj = rollout(&net, &train_states, closes, state_offset, config, charges, &mut rng);
        final_train_reward = traj.returns.first().copied().unwrap_or(0.0);
        let grads = ppo_grad(&net, &traj, config.clip_eps);
        net.apply_adam(&grads, &mut m, &mut v, ep + 1, config.lr);
        if ep % 100 == 0 {
            eprintln!("rl train: episode {}/{} return={:.4}", ep, config.max_episodes, final_train_reward);
        }
    }

    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&net, &val_states, closes, state_offset + train_end, config.allow_short, charges)
    } else { 0.0 };

    eprintln!("rl train: done. val_pnl={:.4}", val_pnl);

    TrainResult { net, final_train_reward, val_pnl, episodes: config.max_episodes }
}

pub fn weights_to_bytes(net: &MLP) -> Vec<u8> {
    net.params().iter().flat_map(|&v| v.to_le_bytes()).collect()
}
