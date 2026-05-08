use ndarray::Array2;
use rand::Rng;
use rand_distr::{Normal, Distribution};

pub struct MLP {
    pub w1: Vec<f64>,
    pub b1: Vec<f64>,
    pub w2: Vec<f64>,
    pub b2: Vec<f64>,
    pub w_mean: Vec<f64>,
    pub b_mean: Vec<f64>,
    pub log_std: Vec<f64>,
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

    fn matmul_bias(w: &[f64], b: &[f64], x: &[f64], rows: usize, cols: usize) -> Vec<f64> {
        (0..rows).map(|i| {
            b[i] + (0..cols).map(|j| w[i * cols + j] * x[j]).sum::<f64>()
        }).collect()
    }

    pub fn forward(&self, x: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let h1: Vec<f64> = Self::matmul_bias(&self.w1, &self.b1, x, self.hidden_size, self.input_size)
            .iter().map(|&v| v.tanh()).collect();
        let h2: Vec<f64> = Self::matmul_bias(&self.w2, &self.b2, &h1, self.hidden_size, self.hidden_size)
            .iter().map(|&v| v.tanh()).collect();
        let mean = Self::matmul_bias(&self.w_mean, &self.b_mean, &h2, self.action_size, self.hidden_size);
        let std: Vec<f64> = self.log_std.iter().map(|&ls| ls.exp().max(0.01)).collect();
        (mean, std)
    }

    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (f64, f64) {
        let (mean, std) = self.forward(x);
        let normal = Normal::new(mean[0], std[0]).unwrap();
        let raw = normal.sample(rng);
        let action = if allow_short { raw.clamp(-1.0, 1.0) } else { raw.clamp(0.0, 1.0) };
        let log_prob = -0.5 * ((raw - mean[0]) / std[0]).powi(2) - std[0].ln() - (2.0 * std::f64::consts::PI).sqrt().ln();
        (action, log_prob)
    }

    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        p.extend_from_slice(&self.w1); p.extend_from_slice(&self.b1);
        p.extend_from_slice(&self.w2); p.extend_from_slice(&self.b2);
        p.extend_from_slice(&self.w_mean); p.extend_from_slice(&self.b_mean);
        p.extend_from_slice(&self.log_std);
        p
    }

    pub fn load_params(&mut self, p: &[f64]) {
        let mut off = 0;
        let mut copy = |dst: &mut Vec<f64>| {
            let n = dst.len(); dst.copy_from_slice(&p[off..off+n]); off += n;
        };
        copy(&mut self.w1); copy(&mut self.b1);
        copy(&mut self.w2); copy(&mut self.b2);
        copy(&mut self.w_mean); copy(&mut self.b_mean);
        copy(&mut self.log_std);
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
    actions: Vec<f64>,
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

fn step_reward(position: f64, prev_price: f64, curr_price: f64, trade_count: usize, holding_candles: usize, config: &TrainConfig) -> f64 {
    let pnl = position * (curr_price - prev_price) / prev_price.max(1e-8);
    let mut reward = pnl;
    if let (Some(max_days), Some(w)) = (config.max_holding_days, config.penalty_holding_days) {
        let holding_days = holding_candles as f64 / 26.0;
        if holding_days > max_days as f64 { reward -= w * (holding_days - max_days as f64); }
    }
    if let (Some(max_trades), Some(w)) = (config.max_trades_per_month, config.penalty_trades_per_month) {
        let monthly_rate = trade_count as f64 / 20.0;
        if monthly_rate > max_trades as f64 { reward -= w * (monthly_rate - max_trades as f64); }
    }
    reward
}

fn rollout(net: &MLP, states: &Array2<f64>, closes: &[f64], state_offset: usize, config: &TrainConfig, rng: &mut impl Rng) -> Trajectory {
    let n = states.nrows().min(config.episode_steps);
    let mut position = 0.0f64;
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
        let prev_price = if ci > 0 { closes[ci - 1] } else { closes[ci] };
        let curr_price = closes[ci];
        if (action - position).abs() > 0.05 { trades += 1; holding = 0; } else { holding += 1; }
        position = action;
        rewards.push(step_reward(position, prev_price, curr_price, trades, holding, config));
        traj_states.push(state);
        traj_actions.push(action);
        traj_log_probs.push(lp);
    }

    let returns = compute_returns(&rewards, config.gamma);
    Trajectory { states: traj_states, actions: traj_actions, log_probs: traj_log_probs, returns }
}

fn ppo_grad(net: &MLP, traj: &Trajectory, clip_eps: f64) -> Vec<f64> {
    let params = net.params();
    let h = 1e-4;
    let eps = 1e-6;

    (0..params.len()).map(|i| {
        let mut p_plus = params.clone();
        let mut p_minus = params.clone();
        p_plus[i] += h;
        p_minus[i] -= h;

        let mut net_p = MLP::new(net.input_size, net.hidden_size, net.action_size);
        net_p.load_params(&p_plus);
        let mut net_m = MLP::new(net.input_size, net.hidden_size, net.action_size);
        net_m.load_params(&p_minus);

        let loss = |n: &MLP| -> f64 {
            traj.states.iter().zip(traj.actions.iter()).zip(traj.log_probs.iter()).zip(traj.returns.iter())
                .map(|(((s, &a), &old_lp), &ret)| {
                    let (mean, std) = n.forward(s);
                    let lp = -0.5 * ((a - mean[0]) / (std[0] + eps)).powi(2) - (std[0] + eps).ln();
                    let ratio = (lp - old_lp).exp().clamp(1.0 - clip_eps, 1.0 + clip_eps);
                    -ratio * ret
                }).sum::<f64>()
        };

        (loss(&net_p) - loss(&net_m)) / (2.0 * h)
    }).collect()
}

pub struct TrainResult {
    pub net: MLP,
    pub final_reward: f64,
    pub episodes: usize,
}

pub fn train(states: &Array2<f64>, closes: &[f64], state_offset: usize, config: &TrainConfig) -> TrainResult {
    let input_size = states.ncols();
    let hidden_size = 64;
    let action_size = 1;
    let mut net = MLP::new(input_size, hidden_size, action_size);
    let param_count = net.param_count();
    let mut m = vec![0.0f64; param_count];
    let mut v = vec![0.0f64; param_count];
    let mut rng = rand::rng();
    let mut final_reward = 0.0f64;

    for ep in 0..config.max_episodes {
        let traj = rollout(&net, states, closes, state_offset, config, &mut rng);
        final_reward = traj.returns.first().copied().unwrap_or(0.0);
        let grads = ppo_grad(&net, &traj, config.clip_eps);
        net.apply_adam(&grads, &mut m, &mut v, ep + 1, config.lr);
        if ep % 100 == 0 {
            eprintln!("rl train: episode {}/{} return={:.4}", ep, config.max_episodes, final_reward);
        }
    }

    TrainResult { net, final_reward, episodes: config.max_episodes }
}

pub fn weights_to_bytes(net: &MLP) -> Vec<u8> {
    net.params().iter().flat_map(|&v| v.to_le_bytes()).collect()
}
