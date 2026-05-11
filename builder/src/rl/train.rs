use ndarray::{Array1, Array2, Axis};
use rand::Rng;
use rayon::prelude::*;

pub type Action = f64;

#[derive(Clone, Debug, PartialEq)]
pub enum Activation {
    Tanh,
    Relu,
}

/// Pure feedforward network with a single output head.
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
    #[allow(dead_code)]
    pub out_size: usize,
}

impl MLP {
    pub fn new(input_size: usize, hidden_size: usize, num_layers: usize, activation: Activation, out_size: usize) -> Self {
        use rand_distr::{Normal, Distribution};
        let mut rng = rand::rng();
        let init_scale = match activation {
            Activation::Relu => (2.0 / input_size as f64).sqrt(),
            Activation::Tanh => (1.0 / input_size as f64).sqrt(),
        };
        let normal = Normal::new(0.0, init_scale).unwrap();
        let mut init = |n: usize| -> Vec<f64> { (0..n).map(|_| normal.sample(&mut rng)).collect() };
        
        let mut layer_weights = vec![];
        let mut layer_biases = vec![];
        for layer_idx in 0..num_layers {
            let prev_size = if layer_idx == 0 { input_size } else { hidden_size };
            let w = Array2::from_shape_vec((hidden_size, prev_size), init(hidden_size * prev_size)).unwrap();
            let b = Array1::zeros(hidden_size);
            layer_weights.push(w);
            layer_biases.push(b);
        }
        
        Self {
            layer_weights,
            layer_biases,
            w_out: Array2::from_shape_vec((out_size, hidden_size), init(out_size * hidden_size)).unwrap(),
            b_out: Array1::zeros(out_size),
            input_size,
            hidden_size,
            num_layers,
            activation,
            out_size,
        }
    }

    pub fn activate(&self, v: f64) -> f64 {
        match self.activation {
            Activation::Tanh => v.tanh(),
            Activation::Relu => v.max(0.0),
        }
    }

    pub fn act_derivative(&self, v: f64) -> f64 {
        match self.activation {
            Activation::Tanh => 1.0 - v * v,
            Activation::Relu => if v > 0.0 { 1.0 } else { 0.0 },
        }
    }

    /// Forward pass. Returns (pre_activations, activations, logits).
    pub fn forward(&self, x: &[f64]) -> (Vec<Array1<f64>>, Vec<Array1<f64>>, Array1<f64>) {
        let mut pre_acts = Vec::with_capacity(self.num_layers);
        let mut acts = Vec::with_capacity(self.num_layers);
        let mut current = Array1::from_vec(x.to_vec());

        let last_layer = self.num_layers.saturating_sub(1);
        for (idx, (w, b)) in self.layer_weights.iter().zip(&self.layer_biases).enumerate() {
            let pre = w.dot(&current) + b;
            let h = pre.mapv(|v| self.activate(v));
            pre_acts.push(pre);
            if idx == last_layer {
                acts.push(h);
            } else {
                acts.push(h.clone());
                current = h;
            }
        }
        
        let logits = self.w_out.dot(&current) + &self.b_out;
        (pre_acts, acts, logits)
    }

    pub fn apply_adam(&mut self, grads: &Grads, m: &mut AdamState, v: &mut AdamState, t: usize, lr: f64) {
        let (beta1, beta2, eps) = (0.9f64, 0.999f64, 1e-8f64);
        let bc1 = 1.0 - beta1.powi(t as i32);
        let bc2 = 1.0 - beta2.powi(t as i32);

        fn update(param: &mut [f64], grad: &[f64], m: &mut [f64], v: &mut [f64], beta1: f64, beta2: f64, lr: f64, bc1: f64, bc2: f64, eps: f64) {
            for i in 0..param.len() {
                m[i] = beta1 * m[i] + (1.0 - beta1) * grad[i];
                v[i] = beta2 * v[i] + (1.0 - beta2) * grad[i].powi(2);
                param[i] -= lr * (m[i] / bc1) / ((v[i] / bc2).sqrt() + eps);
            }
        }

        for i in 0..self.layer_weights.len() {
            update(
                self.layer_weights[i].as_slice_mut().unwrap(),
                grads.layer_weights[i].as_slice().unwrap(),
                m.layer_weights[i].as_slice_mut().unwrap(),
                v.layer_weights[i].as_slice_mut().unwrap(),
                beta1, beta2, lr, bc1, bc2, eps
            );
            update(
                self.layer_biases[i].as_slice_mut().unwrap(),
                grads.layer_biases[i].as_slice().unwrap(),
                m.layer_biases[i].as_slice_mut().unwrap(),
                v.layer_biases[i].as_slice_mut().unwrap(),
                beta1, beta2, lr, bc1, bc2, eps
            );
        }
        update(self.w_out.as_slice_mut().unwrap(), grads.w_out.as_slice().unwrap(), m.w_out.as_slice_mut().unwrap(), v.w_out.as_slice_mut().unwrap(), beta1, beta2, lr, bc1, bc2, eps);
        update(self.b_out.as_slice_mut().unwrap(), grads.b_out.as_slice().unwrap(), m.b_out.as_slice_mut().unwrap(), v.b_out.as_slice_mut().unwrap(), beta1, beta2, lr, bc1, bc2, eps);
    }

    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        for (w, b) in self.layer_weights.iter().zip(&self.layer_biases) {
            p.extend(w.iter());
            p.extend(b.iter());
        }
        p.extend(self.w_out.iter());
        p.extend(self.b_out.iter());
        p
    }

    pub fn add_regularization(&self, grads: &mut Grads, reg_type: &str, lambda: f64) {
        if lambda <= 0.0 || reg_type == "none" {
            return;
        }
        match reg_type {
            "l1" => {
                for (layer_idx, w) in self.layer_weights.iter().enumerate() {
                    grads.layer_weights[layer_idx] += &w.mapv(|v| lambda * v.signum());
                }
                grads.w_out += &self.w_out.mapv(|v| lambda * v.signum());
            }
            "l2" => {
                for (layer_idx, w) in self.layer_weights.iter().enumerate() {
                    grads.layer_weights[layer_idx] += &(w * lambda);
                }
                grads.w_out += &(self.w_out.clone() * lambda);
            }
            _ => {}
        }
    }
}

/// Policy network.
#[derive(Clone)]
pub struct Actor {
    pub net: MLP,
    pub continuous_action: bool,
    pub action_std: f64,
}

impl Actor {
    pub fn new(input_size: usize, hidden_size: usize, num_layers: usize, activation: Activation, continuous_action: bool, action_std: f64) -> Self {
        let out_size = if continuous_action { 1 } else { 3 };
        Self {
            net: MLP::new(input_size, hidden_size, num_layers, activation, out_size),
            continuous_action,
            action_std,
        }
    }

    pub fn forward_full(&self, x: &[f64]) -> (Vec<Array1<f64>>, Vec<Array1<f64>>, [f64; 3]) {
        let (pre_acts, acts, logits) = self.net.forward(x);
        if self.continuous_action {
            let mean = logits[0].tanh();
            (pre_acts, acts, [mean, 0.0, 0.0])
        } else {
            let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let exp = logits.mapv(|v| (v - max).exp());
            let sum: f64 = exp.sum();
            let probs = [exp[0] / sum, exp[1] / sum, exp[2] / sum];
            (pre_acts, acts, probs)
        }
    }

    pub fn probs(&self, x: &[f64], mask_sell: bool) -> [f64; 3] {
        let (_, _, mut p) = self.forward_full(x);
        if !self.continuous_action && mask_sell {
            let s = p[0] + p[1];
            p[0] /= s; p[1] /= s; p[2] = 0.0;
        }
        p
    }

    pub fn sample_action(&self, x: &[f64], rng: &mut impl Rng, allow_short: bool) -> (Action, f64) {
        if self.continuous_action {
            let mean = self.forward_full(x).2[0];
            let std = self.action_std;
            let u1: f64 = rng.random();
            let u2: f64 = rng.random();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            let action = (mean + std * z).clamp(if allow_short { -1.0 } else { 0.0 }, 1.0);
            let log_prob = -0.5 * ((action - mean) / std).powi(2) - std.ln() - 0.5 * (2.0 * std::f64::consts::PI).ln();
            (action, log_prob)
        } else {
            let p = self.probs(x, !allow_short);
            let r: f64 = rng.random();
            let action = if r < p[0] { 0.0 } else if r < p[0] + p[1] { 1.0 } else { 2.0 };
            (action, p[action as usize].max(1e-12).ln())
        }
    }

    pub fn greedy_action(&self, x: &[f64], allow_short: bool) -> Action {
        if self.continuous_action {
            let mean = self.forward_full(x).2[0];
            mean.clamp(if allow_short { -1.0 } else { 0.0 }, 1.0)
        } else {
            let p = self.probs(x, !allow_short);
            if p[0] >= p[1] && p[0] >= p[2] { 0.0 } else if p[1] >= p[2] { 1.0 } else { 2.0 }
        }
    }

    pub fn accumulate_grad(&self, x: &[f64], action: Action, advantage: f64, grads: &mut Grads) {
        let (pre_acts, acts, probs) = self.forward_full(x);
        let h_last = acts.last().unwrap();

        if self.continuous_action {
            let mean = probs[0];
            let std = self.action_std;
            let d_mean = -advantage * (action - mean) / (std * std) * (1.0 - mean * mean);
            grads.w_out += &(h_last.clone().insert_axis(Axis(0)) * d_mean);
            grads.b_out[0] += d_mean;

            let mut d_h = &self.net.w_out.row(0) * d_mean;

            for layer_idx in (0..self.net.num_layers).rev() {
                let pre = &pre_acts[layer_idx];
                let prev_h = if layer_idx == 0 { Array1::from(x.to_vec()) } else { acts[layer_idx - 1].clone() };
                let d_pre = pre.mapv(|p| self.net.act_derivative(p)) * &d_h;

                for (a, b) in grads.layer_biases[layer_idx].iter_mut().zip(d_pre.iter()) {
                    *a += b;
                }
                let dw = d_pre.clone().insert_axis(Axis(1)) * prev_h.insert_axis(Axis(0));
                for (a, b) in grads.layer_weights[layer_idx].iter_mut().zip(dw.iter()) {
                    *a += b;
                }

                if layer_idx > 0 {
                    d_h = self.net.layer_weights[layer_idx].t().dot(&d_pre);
                }
            }
            return;
        }

        let mut d_logits = Array1::zeros(3);
        for k in 0..3 {
            let indicator = if k == action as usize { 1.0 } else { 0.0 };
            d_logits[k] = -advantage * (indicator - probs[k]);
        }

        grads.b_out += &d_logits;
        grads.w_out += &(h_last.clone().insert_axis(Axis(0)) * d_logits.clone().insert_axis(Axis(1)));

        let mut d_h = self.net.w_out.t().dot(&d_logits);

        for layer_idx in (0..self.net.num_layers).rev() {
            let pre = &pre_acts[layer_idx];
            let prev_h = if layer_idx == 0 { Array1::from(x.to_vec()) } else { acts[layer_idx - 1].clone() };
            let d_pre = pre.mapv(|p| self.net.act_derivative(p)) * &d_h;

            for (a, b) in grads.layer_biases[layer_idx].iter_mut().zip(d_pre.iter()) {
                *a += b;
            }
            let dw = d_pre.clone().insert_axis(Axis(1)) * prev_h.insert_axis(Axis(0));
            for (a, b) in grads.layer_weights[layer_idx].iter_mut().zip(dw.iter()) {
                *a += b;
            }

            if layer_idx > 0 {
                d_h = self.net.layer_weights[layer_idx].t().dot(&d_pre);
            }
        }
    }

    /// Accumulate PPO policy gradient for a single transition.
    pub fn accumulate_grad_ppo(
        &self,
        x: &[f64],
        action: Action,
        old_log_prob: f64,
        advantage: f64,
        clip_epsilon: f64,
        entropy_coef: f64,
        grads: &mut Grads,
    ) {
        let (pre_acts, acts, probs) = self.forward_full(x);
        let h_last = acts.last().unwrap();

        if self.continuous_action {
            let mean = probs[0];
            let std = self.action_std;
            let diff = action - mean;
            let new_log_prob = -0.5 * (diff / std).powi(2) - std.ln() - 0.5 * (2.0 * std::f64::consts::PI).ln();
            let ratio = (new_log_prob - old_log_prob).exp();

            let clipped = ratio.clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon);
            let use_clipped = ratio * advantage > clipped * advantage;

            let mut d_mean = 0.0;
            if !use_clipped {
                d_mean = -advantage * ratio * diff / (std * std);
            }
            d_mean *= 1.0 - mean * mean;

            grads.w_out += &(h_last.clone().insert_axis(Axis(0)) * d_mean);
            grads.b_out[0] += d_mean;

            let mut d_h = &self.net.w_out.row(0) * d_mean;

            for layer_idx in (0..self.net.num_layers).rev() {
                let pre = &pre_acts[layer_idx];
                let prev_h = if layer_idx == 0 { Array1::from(x.to_vec()) } else { acts[layer_idx - 1].clone() };
                let d_pre = pre.mapv(|p| self.net.act_derivative(p)) * &d_h;

                for (a, b) in grads.layer_biases[layer_idx].iter_mut().zip(d_pre.iter()) {
                    *a += b;
                }
                let dw = d_pre.clone().insert_axis(Axis(1)) * prev_h.insert_axis(Axis(0));
                for (a, b) in grads.layer_weights[layer_idx].iter_mut().zip(dw.iter()) {
                    *a += b;
                }

                if layer_idx > 0 {
                    d_h = self.net.layer_weights[layer_idx].t().dot(&d_pre);
                }
            }
            return;
        }

        let action_idx = action as usize;
        let new_log_prob = probs[action_idx].max(1e-12).ln();
        let ratio = (new_log_prob - old_log_prob).exp();

        let clipped = ratio.clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon);
        let use_clipped = ratio * advantage > clipped * advantage;

        let mut d_logits = Array1::zeros(3);
        if !use_clipped {
            for k in 0..3 {
                let indicator = if k == action_idx { 1.0 } else { 0.0 };
                d_logits[k] = -advantage * ratio * (indicator - probs[k]);
            }
        }
        for k in 0..3 {
            if probs[k] > 1e-12 {
                d_logits[k] += entropy_coef * probs[k] * (probs[k].ln() + 1.0);
            }
        }

        grads.b_out += &d_logits;
        grads.w_out += &(h_last.clone().insert_axis(Axis(0)) * d_logits.clone().insert_axis(Axis(1)));

        let mut d_h = self.net.w_out.t().dot(&d_logits);

        for layer_idx in (0..self.net.num_layers).rev() {
            let pre = &pre_acts[layer_idx];
            let prev_h = if layer_idx == 0 { Array1::from(x.to_vec()) } else { acts[layer_idx - 1].clone() };
            let d_pre = pre.mapv(|p| self.net.act_derivative(p)) * &d_h;

            for (a, b) in grads.layer_biases[layer_idx].iter_mut().zip(d_pre.iter()) {
                *a += b;
            }
            let dw = d_pre.clone().insert_axis(Axis(1)) * prev_h.insert_axis(Axis(0));
            for (a, b) in grads.layer_weights[layer_idx].iter_mut().zip(dw.iter()) {
                *a += b;
            }

            if layer_idx > 0 {
                d_h = self.net.layer_weights[layer_idx].t().dot(&d_pre);
            }
        }
    }
}

/// Value network.
#[derive(Clone)]
pub struct Critic {
    pub net: MLP,
}

impl Critic {
    pub fn new(input_size: usize, hidden_size: usize, num_layers: usize, activation: Activation) -> Self {
        Self {
            net: MLP::new(input_size, hidden_size, num_layers, activation, 1),
        }
    }

    pub fn forward_value(&self, x: &[f64]) -> f64 {
        let (_, _, logits) = self.net.forward(x);
        logits[0]
    }

    pub fn accumulate_grad(&self, x: &[f64], ret: f64, grads: &mut Grads) {
        let (pre_acts, acts, logits) = self.net.forward(x);
        let value = logits[0];
        let d_value = value - ret; // gradient of 0.5 * (value - ret)^2

        let h_last = acts.last().unwrap();
        grads.w_out += &(h_last.clone().insert_axis(Axis(0)) * d_value);
        grads.b_out[0] += d_value;

        let mut d_h = self.net.w_out.row(0).to_owned() * d_value;

        for layer_idx in (0..self.net.num_layers).rev() {
            let pre = &pre_acts[layer_idx];
            let prev_h = if layer_idx == 0 { Array1::from(x.to_vec()) } else { acts[layer_idx - 1].clone() };
            let d_pre = pre.mapv(|p| self.net.act_derivative(p)) * &d_h;

            for (a, b) in grads.layer_biases[layer_idx].iter_mut().zip(d_pre.iter()) {
                *a += b;
            }
            let dw = d_pre.clone().insert_axis(Axis(1)) * prev_h.insert_axis(Axis(0));
            for (a, b) in grads.layer_weights[layer_idx].iter_mut().zip(dw.iter()) {
                *a += b;
            }

            if layer_idx > 0 {
                d_h = self.net.layer_weights[layer_idx].t().dot(&d_pre);
            }
        }
    }
}

pub struct Grads {
    pub layer_weights: Vec<Array2<f64>>,
    pub layer_biases: Vec<Array1<f64>>,
    pub w_out: Array2<f64>,
    pub b_out: Array1<f64>,
}

impl Grads {
    pub fn zero(net: &MLP) -> Self {
        Self {
            layer_weights: net.layer_weights.iter().map(|w| Array2::zeros(w.raw_dim())).collect(),
            layer_biases: net.layer_biases.iter().map(|b| Array1::zeros(b.raw_dim())).collect(),
            w_out: Array2::zeros(net.w_out.raw_dim()),
            b_out: Array1::zeros(net.b_out.raw_dim()),
        }
    }

    pub fn clip_global_norm(&mut self, max_norm: f64) {
        let mut sum_sq = 0.0f64;
        for w in &self.layer_weights {
            sum_sq += w.iter().map(|v| v * v).sum::<f64>();
        }
        for b in &self.layer_biases {
            sum_sq += b.iter().map(|v| v * v).sum::<f64>();
        }
        sum_sq += self.w_out.iter().map(|v| v * v).sum::<f64>();
        sum_sq += self.b_out.iter().map(|v| v * v).sum::<f64>();
        let norm = sum_sq.sqrt();
        if norm <= max_norm || norm < 1e-12 {
            return;
        }
        let scale = max_norm / norm;
        for w in &mut self.layer_weights {
            *w *= scale;
        }
        for b in &mut self.layer_biases {
            *b *= scale;
        }
        self.w_out *= scale;
        self.b_out *= scale;
    }
}

pub struct AdamState {
    pub layer_weights: Vec<Array2<f64>>,
    pub layer_biases: Vec<Array1<f64>>,
    pub w_out: Array2<f64>,
    pub b_out: Array1<f64>,
}

impl AdamState {
    pub fn zero(net: &MLP) -> Self {
        Self {
            layer_weights: net.layer_weights.iter().map(|w| Array2::zeros(w.raw_dim())).collect(),
            layer_biases: net.layer_biases.iter().map(|b| Array1::zeros(b.raw_dim())).collect(),
            w_out: Array2::zeros(net.w_out.raw_dim()),
            b_out: Array1::zeros(net.b_out.raw_dim()),
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
    pub fn cost(&self, buy_price: f64, sell_price: f64, size: f64) -> f64 {
        let trade_value_buy = buy_price * size;
        let trade_value_sell = sell_price * size;
        let brokerage = (self.brokerage_pct * trade_value_buy).min(self.brokerage_flat)
            + (self.brokerage_pct * trade_value_sell).min(self.brokerage_flat);
        let stt = self.stt_buy_pct * trade_value_buy + self.stt_sell_pct * trade_value_sell;
        let exchange = self.exchange_pct * (trade_value_buy + trade_value_sell);
        let sebi = self.sebi_pct * (trade_value_buy + trade_value_sell);
        let stamp = self.stamp_buy_pct * trade_value_buy;
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
    pub actor_lr: f64,
    pub critic_lr: f64,
    pub gamma: f64,
    pub allow_short: bool,
    pub reward_type: String,
    pub penalty_holding_days: Option<f64>,
    pub max_holding_days: Option<usize>,
    pub penalty_trades_per_month: Option<f64>,
    pub max_trades_per_month: Option<usize>,
    pub ppo_epochs: usize,
    pub clip_epsilon: f64,
    pub value_coef: f64,
    pub entropy_coef: f64,
    pub gae_lambda: f64,
    pub batch_episodes: usize,
    pub minibatch_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub activation: String,
    pub reward_norm: bool,
    pub lr_schedule: bool,
    pub entropy_anneal: bool,
    pub regularization_type: String,
    pub regularization_lambda: f64,
    pub continuous_action: bool,
    pub action_std: f64,
    pub action_std_schedule: bool,
    pub action_penalty: f64,
    pub position_deadband: f64,
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
            lr: 1e-4,
            actor_lr: 1e-4,
            critic_lr: 1e-3,
            gamma: 0.99,
            allow_short: false,
            reward_type: "pnl".into(),
            penalty_holding_days: None,
            max_holding_days: None,
            penalty_trades_per_month: None,
            max_trades_per_month: None,
            ppo_epochs: 4,
            clip_epsilon: 0.2,
            // Reduced from 0.5 to mitigate shared network interference.
            // Full fix: separate Actor and Critic into distinct MLPs.
            value_coef: 0.25,
            entropy_coef: 0.01,
            gae_lambda: 0.95,
            batch_episodes: 8,
            minibatch_size: 64,
            hidden_size: 64,
            num_layers: 4,
            activation: "relu".into(),
            reward_norm: true,
            lr_schedule: true,
            entropy_anneal: true,
            regularization_type: "l2".into(),
            regularization_lambda: 0.001,
            continuous_action: false,
            action_std: 0.3,
            action_std_schedule: true,
            action_penalty: 0.01,
            position_deadband: 0.05,
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
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    let std = var.sqrt().max(1e-8);
    returns.iter().map(|r| (r - mean) / std).collect()
}

fn lr_at_step(initial_lr: f64, step: usize, total_steps: usize) -> f64 {
    if total_steps == 0 { return initial_lr; }
    let frac = step as f64 / total_steps as f64;
    initial_lr * (1.0 - frac)
}

fn entropy_at_step(initial_entropy: f64, step: usize, total_steps: usize) -> f64 {
    if total_steps == 0 { return initial_entropy; }
    let frac = step as f64 / total_steps as f64;
    (initial_entropy * (1.0 - frac)).max(0.001)
}

fn action_std_at_step(initial_std: f64, step: usize, total_steps: usize) -> f64 {
    if total_steps == 0 { return initial_std; }
    let frac = step as f64 / total_steps as f64;
    // Anneal from initial_std down to 1% of initial_std
    initial_std * (1.0 - 0.99 * frac).max(0.01)
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
    prev_action: Action,
    position: &mut f64,
    entry_price: &mut f64,
    holding: &mut usize,
    trades: &mut usize,
    price: f64,
    prev_price: f64,
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> f64 {
    let mut reward = 0.0;

    // Reward shaping: unrealized PnL from price movement while holding position
    reward += *position * (price - prev_price);

    // Reward shaping: penalize large position changes (action smoothing)
    if config.action_penalty > 0.0 {
        reward -= config.action_penalty * (action - prev_action).abs();
    }

    if config.continuous_action {
        let prev_pos = *position;
        let delta = action - prev_pos;

        if delta.abs() >= config.position_deadband {
            // Close/reduced portion: only subtract costs (MTM already captured the PnL)
            if prev_pos > 0.0 && action < prev_pos {
                let closed = prev_pos - action.max(0.0);
                reward -= charges.cost(*entry_price, price, closed);
                *trades += 1;
            } else if prev_pos < 0.0 && action > prev_pos {
                let closed = prev_pos.abs() - action.min(0.0).abs();
                reward -= charges.cost(price, *entry_price, closed);
                *trades += 1;
            }

            // Update entry_price
            if action == 0.0 {
                *entry_price = 0.0;
            } else if prev_pos == 0.0 {
                *entry_price = price;
                *trades += 1;
            } else if prev_pos.signum() != action.signum() {
                // Flip direction
                *entry_price = price;
                *trades += 1;
            } else {
                // Same direction
                if action.abs() > prev_pos.abs() {
                    // Adding to position: update weighted average
                    let same_dir_pos = if prev_pos > 0.0 { prev_pos.min(action) } else { prev_pos.max(action) };
                    let added = delta.abs();
                    let new_size = action.abs();
                    if new_size > 0.0 {
                        *entry_price = (*entry_price * same_dir_pos.abs() + price * added) / new_size;
                    }
                    *trades += 1;
                }
                // If reducing, entry_price stays the same for remaining shares
            }

            *position = action;
        }

        if *position != 0.0 { *holding += 1; } else { *holding = 0; }
    } else {
        // Discrete action space (backward compatible)
        match (*position, action) {
            (0.0, 1.0) => {
                *position = 1.0; *entry_price = price; *holding = 0; *trades += 1;
            }
            (0.0, 2.0) if config.allow_short => {
                *position = -1.0; *entry_price = price; *holding = 0; *trades += 1;
            }
            (1.0, 0.0) | (1.0, 2.0) => {
                // MTM already captured the PnL; only subtract costs
                reward -= charges.cost(*entry_price, price, 1.0);
                *trades += 1;
                *position = 0.0; *holding = 0;
                if action == 2.0 && config.allow_short {
                    *position = -1.0; *entry_price = price; *trades += 1;
                }
            }
            (-1.0, 0.0) | (-1.0, 1.0) => {
                // MTM already captured the PnL; only subtract costs
                reward -= charges.cost(price, *entry_price, 1.0);
                *trades += 1;
                *position = 0.0; *holding = 0;
                if action == 1.0 {
                    *position = 1.0; *entry_price = price; *trades += 1;
                }
            }
            _ => {}
        }

        if *position != 0.0 { *holding += 1; }
    }

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

fn position_features(position: f64, holding: usize, entry_price: f64, price: f64) -> [f64; 3] {
    let unrealized = if position > 0.0 {
        position * (price - entry_price)
    } else if position < 0.0 {
        position.abs() * (entry_price - price)
    } else {
        0.0
    };
    [
        position,
        (holding as f64 / 20.0).clamp(-5.0, 5.0),
        (unrealized / entry_price.max(1e-8)).clamp(-5.0, 5.0),
    ]
}

fn decision_state(base_state: &[f64], position: f64, holding: usize, entry_price: f64, price: f64) -> Vec<f64> {
    let mut state = Vec::with_capacity(base_state.len() + 3);
    state.extend_from_slice(base_state);
    state.extend_from_slice(&position_features(position, holding, entry_price, price));
    state
}

fn close_position_reward(position: f64, entry_price: f64, price: f64, charges: &BrokerCharges) -> f64 {
    let size = position.abs();
    let gross = if position > 0.0 {
        size * (price - entry_price)
    } else {
        size * (entry_price - price)
    };
    let (buy, sell) = if position > 0.0 { (entry_price, price) } else { (price, entry_price) };
    gross - charges.cost(buy, sell, size)
}

fn greedy_objective(
    actor: &Actor,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
) -> f64 {
    let n = states.nrows();
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut rewards = vec![];
    let mut prev_action: Action = 0.0;

    for t in 0..n {
        let base_state = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let state = decision_state(&base_state, position, holding, entry_price, price);
        let action = actor.greedy_action(&state, config.allow_short);
        let prev_price = if t > 0 { closes[candle_indices[t - 1]] } else { price };

        let r = step_reward(action, prev_action, &mut position, &mut entry_price, &mut holding, &mut trades, price, prev_price, config, charges);
        rewards.push(r);
        prev_action = action;
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

pub fn collect_greedy_states(
    actor: &Actor,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    allow_short: bool,
) -> Array2<f64> {
    let n = states.nrows();
    let base_dim = states.ncols();
    let state_dim = base_dim + 3;
    let mut out = Array2::zeros((n, state_dim));
    let mut position: f64 = 0.0;
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

        let action = actor.greedy_action(
            &decision_state(&base, position, holding, entry_price, price),
            allow_short,
        );

        if actor.continuous_action {
            if action != position {
                entry_price = if action != 0.0 { price } else { 0.0 };
            }
            position = action;
            holding = if position != 0.0 { holding + 1 } else { 0 };
        } else {
            match (position, action) {
                (0.0, 1.0) => { position = 1.0; entry_price = price; holding = 0; }
                (0.0, 2.0) if allow_short => { position = -1.0; entry_price = price; holding = 0; }
                (1.0, 0.0) | (1.0, 2.0) => {
                    position = 0.0; holding = 0;
                    if action == 2.0 && allow_short { position = -1.0; entry_price = price; }
                }
                (-1.0, 0.0) | (-1.0, 1.0) => {
                    position = 0.0; holding = 0;
                    if action == 1.0 { position = 1.0; entry_price = price; }
                }
                _ => {}
            }
            if position != 0.0 { holding += 1; }
        }
    }
    out
}

fn rollout(
    actor: &Actor,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
    rng: &mut impl Rng,
) -> (Vec<Step>, f64) {
    let n = states.nrows().min(config.episode_steps);
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut steps = vec![];
    let mut rewards = vec![];
    let mut prev_action: Action = 0.0;

    let start = if states.nrows() > n {
        rng.random_range(0..=(states.nrows() - n))
    } else {
        0
    };

    for t in 0..n {
        let row_idx = start + t;
        let price = closes[candle_indices[row_idx]];
        let state = decision_state(&states.row(row_idx).to_vec(), position, holding, entry_price, price);
        let (action, _) = actor.sample_action(&state, rng, config.allow_short);
        let prev_price = if t > 0 { closes[candle_indices[row_idx - 1]] } else { price };
        let r = step_reward(action, prev_action, &mut position, &mut entry_price, &mut holding, &mut trades, price, prev_price, config, charges);
        steps.push(Step { state, action, ret: 0.0 });
        rewards.push(r);
        prev_action = action;
    }

    // End-of-episode fee evasion fix: force-close any open position and subtract costs
    if position != 0.0 && n > 0 {
        let last_price = closes[candle_indices[start + n - 1]];
        let close_cost = if position > 0.0 {
            charges.cost(entry_price, last_price, position)
        } else {
            charges.cost(last_price, entry_price, position.abs())
        };
        if let Some(last_reward) = rewards.last_mut() {
            *last_reward -= close_cost;
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

fn gae(rewards: &[f64], values: &[f64], gamma: f64, lambda: f64, bootstrap_value: f64) -> Vec<f64> {
    let n = rewards.len();
    let mut advantages = vec![0.0; n];
    let mut gae = 0.0;
    for t in (0..n).rev() {
        let next_value = if t + 1 < n { values[t + 1] } else { bootstrap_value };
        let delta = rewards[t] + gamma * next_value - values[t];
        gae = delta + gamma * lambda * gae;
        advantages[t] = gae;
    }
    advantages
}

struct TrajectoryStep {
    state: Vec<f64>,
    action: Action,
    log_prob: f64,
    value: f64,
    reward: f64,
}

fn rollout_ppo(
    actor: &Actor,
    critic: &Critic,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
    rng: &mut impl Rng,
) -> (Vec<TrajectoryStep>, f64, f64) {
    let n = states.nrows().min(config.episode_steps);
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut trades = 0usize;
    let mut steps = vec![];
    let mut rewards = vec![];
    let mut prev_action: Action = 0.0;

    let start = if states.nrows() > n {
        rng.random_range(0..=(states.nrows() - n))
    } else {
        0
    };

    for t in 0..n {
        let row_idx = start + t;
        let price = closes[candle_indices[row_idx]];
        let state = decision_state(&states.row(row_idx).to_vec(), position, holding, entry_price, price);
        let (action, log_prob) = actor.sample_action(&state, rng, config.allow_short);
        let value = critic.forward_value(&state);
        let prev_price = if t > 0 { closes[candle_indices[row_idx - 1]] } else { price };
        let r = step_reward(action, prev_action, &mut position, &mut entry_price, &mut holding, &mut trades, price, prev_price, config, charges);
        steps.push(TrajectoryStep { state, action, log_prob, value, reward: r });
        rewards.push(r);
        prev_action = action;
    }

    // End-of-episode fee evasion fix: force-close any open position and subtract costs
    if position != 0.0 && n > 0 {
        let last_price = closes[candle_indices[start + n - 1]];
        let close_cost = if position > 0.0 {
            charges.cost(entry_price, last_price, position)
        } else {
            charges.cost(last_price, entry_price, position.abs())
        };
        if let Some(last_reward) = rewards.last_mut() {
            *last_reward -= close_cost;
        }
        if let Some(last_step) = steps.last_mut() {
            last_step.reward -= close_cost;
        }
    }

    // Bootstrap GAE from the value of the final state
    let final_row_idx = start + n - 1;
    let final_price = closes[candle_indices[final_row_idx]];
    let final_state = decision_state(&states.row(final_row_idx).to_vec(), position, holding, entry_price, final_price);
    let bootstrap_value = critic.forward_value(&final_state);

    let episode_return = match config.reward_type.as_str() {
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
    };
    (steps, episode_return, bootstrap_value)
}

#[derive(Clone, Debug)]
pub struct EpisodeMetric {
    pub episode: usize,
    pub train_reward: f64,
    pub val_metric: Option<f64>,
}

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

pub fn train_reinforce(
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
    let hidden_size = config.hidden_size;
    let activation = match config.activation.as_str() {
        "tanh" => Activation::Tanh,
        _ => Activation::Relu,
    };
    let mut actor = Actor::new(input_size, hidden_size, config.num_layers, activation, config.continuous_action, config.action_std);
    let mut m = AdamState::zero(&actor.net);
    let mut v = AdamState::zero(&actor.net);
    let mut rng = rand::rng();
    let mut final_train_reward = 0.0f64;
    let mut reward_ema: Option<f64> = None;
    let mut best_actor = actor.clone();
    let mut best_val_metric = f64::NEG_INFINITY;
    let mut best_episode = 0usize;
    let mut episodes_since_best = 0usize;
    let mut episodes_run = 0usize;
    let mut metrics: Vec<EpisodeMetric> = vec![];
    let validation_interval = config.validation_interval.max(1);

    for ep in 0..config.max_episodes {
        episodes_run = ep + 1;
        if config.continuous_action && config.action_std_schedule {
            actor.action_std = action_std_at_step(config.action_std, ep, config.max_episodes);
        }
        let (steps, episode_return) = rollout(&actor, &train_states, closes, train_indices, config, charges, &mut rng);
        reward_ema = Some(match reward_ema {
            Some(prev) => 0.95 * prev + 0.05 * episode_return,
            None => episode_return,
        });
        final_train_reward = reward_ema.unwrap_or(episode_return);

        let mut grads = Grads::zero(&actor.net);
        for step in &steps {
            actor.accumulate_grad(&step.state, step.action, step.ret, &mut grads);
        }
        let n_steps = steps.len() as f64;
        for w in &mut grads.layer_weights {
            for x in w.iter_mut() { *x /= n_steps; }
        }
        for b in &mut grads.layer_biases {
            for x in b.iter_mut() { *x /= n_steps; }
        }
        macro_rules! norm { ($v:expr) => { for x in $v.iter_mut() { *x /= n_steps; } }; }
        norm!(grads.w_out); norm!(grads.b_out);
        actor.net.add_regularization(&mut grads, &config.regularization_type, config.regularization_lambda);
        grads.clip_global_norm(config.grad_clip_norm);

        let actor_lr = if config.lr_schedule {
            lr_at_step(config.actor_lr, ep, config.max_episodes)
        } else {
            config.actor_lr
        };
        actor.net.apply_adam(&grads, &mut m, &mut v, ep + 1, actor_lr);

        eprintln!("rl reinforce: episode {}/{} return={:.4}", ep, config.max_episodes, final_train_reward);

        let mut val_metric_opt: Option<f64> = None;
        if val_states.nrows() > 0 && ((ep + 1) % validation_interval == 0 || ep + 1 == config.max_episodes) {
            let val_metric = greedy_objective(&actor, &val_states, closes, val_indices, config, charges);
            val_metric_opt = Some(val_metric);
            if val_metric > best_val_metric + config.min_delta {
                best_val_metric = val_metric;
                best_actor = actor.clone();
                best_episode = ep + 1;
                episodes_since_best = 0;
            } else {
                episodes_since_best += validation_interval;
                if episodes_since_best >= config.early_stopping_patience {
                    eprintln!("rl reinforce: early stopping at episode {} best_episode={} best_val_metric={:.4}", ep + 1, best_episode, best_val_metric);
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
        actor = best_actor;
    }

    let train_pnl = evaluate(&actor, &train_states, closes, train_indices, config.allow_short, charges, config.position_deadband);
    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&actor, &val_states, closes, val_indices, config.allow_short, charges, config.position_deadband)
    } else { 0.0 };
    let test_pnl = if test_states.nrows() > 0 {
        evaluate(&actor, &test_states, closes, test_indices, config.allow_short, charges, config.position_deadband)
    } else { 0.0 };
    eprintln!("rl reinforce: done. train_pnl={:.4} val_pnl={:.4} test_pnl={:.4}", train_pnl, val_pnl, test_pnl);
    TrainResult { actor, final_train_reward, train_pnl, val_pnl, test_pnl, episodes: episodes_run, best_episode, metrics }
}

pub fn train_ppo(
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
    let hidden_size = config.hidden_size;
    let activation = match config.activation.as_str() {
        "tanh" => Activation::Tanh,
        _ => Activation::Relu,
    };
    let mut actor = Actor::new(input_size, hidden_size, config.num_layers, activation.clone(), config.continuous_action, config.action_std);
    let mut critic = Critic::new(input_size, hidden_size, config.num_layers, activation);
    let mut actor_m = AdamState::zero(&actor.net);
    let mut actor_v = AdamState::zero(&actor.net);
    let mut critic_m = AdamState::zero(&critic.net);
    let mut critic_v = AdamState::zero(&critic.net);
    let mut rng = rand::rng();
    let mut best_actor = actor.clone();
    let mut best_val_metric = f64::NEG_INFINITY;
    let mut best_iteration = 0usize;
    let mut iterations_since_best = 0usize;
    let mut total_episodes = 0usize;
    let mut metrics: Vec<EpisodeMetric> = vec![];
    let validation_interval = config.validation_interval.max(1);
    let ppo_epochs = config.ppo_epochs.max(1);
    let clip_epsilon = config.clip_epsilon;
    let gae_lambda = config.gae_lambda;
    let batch_episodes = config.batch_episodes.max(1);

    for iteration in 0..config.max_episodes {
        let current_entropy_coef = if config.entropy_anneal {
            entropy_at_step(config.entropy_coef, iteration, config.max_episodes)
        } else {
            config.entropy_coef
        };
        let current_actor_lr = if config.lr_schedule {
            lr_at_step(config.actor_lr, iteration, config.max_episodes)
        } else {
            config.actor_lr
        };
        let current_critic_lr = if config.lr_schedule {
            lr_at_step(config.critic_lr, iteration, config.max_episodes)
        } else {
            config.critic_lr
        };
        if config.continuous_action && config.action_std_schedule {
            actor.action_std = action_std_at_step(config.action_std, iteration, config.max_episodes);
        }

        let mut batch_states: Vec<Vec<f64>> = vec![];
        let mut batch_actions: Vec<Action> = vec![];
        let mut batch_log_probs: Vec<f64> = vec![];
        let mut batch_advantages: Vec<f64> = vec![];
        let mut batch_returns: Vec<f64> = vec![];
        let mut episode_returns = vec![];

        let rollouts: Vec<_> = (0..batch_episodes)
            .into_par_iter()
            .map(|_| {
                let mut rng = rand::rng();
                let (traj, ep_return, bootstrap_value) = rollout_ppo(&actor, &critic, &train_states, closes, train_indices, config, charges, &mut rng);
                let traj_rewards: Vec<f64> = traj.iter().map(|t| t.reward).collect();
                let traj_values: Vec<f64> = traj.iter().map(|t| t.value).collect();
                let advantages = gae(&traj_rewards, &traj_values, config.gamma, gae_lambda, bootstrap_value);
                let returns: Vec<f64> = advantages.iter().zip(&traj_values).map(|(a, v)| a + v).collect();
                let steps: Vec<_> = traj.into_iter().zip(advantages.into_iter().zip(returns.into_iter())).collect();
                (steps, ep_return)
            })
            .collect();

        for (steps, ep_return) in rollouts {
            episode_returns.push(ep_return);
            total_episodes += 1;
            for (t, (adv, ret)) in steps {
                batch_states.push(t.state);
                batch_actions.push(t.action);
                batch_log_probs.push(t.log_prob);
                batch_advantages.push(adv);
                batch_returns.push(ret);
            }
        }

        let adv_mean = batch_advantages.iter().sum::<f64>() / batch_advantages.len() as f64;
        let adv_std = (batch_advantages.iter().map(|a| (a - adv_mean).powi(2)).sum::<f64>() / batch_advantages.len() as f64).sqrt().max(1e-8);
        let normalized_advantages: Vec<f64> = batch_advantages.iter().map(|a| (a - adv_mean) / adv_std).collect();

        let train_reward = episode_returns.iter().sum::<f64>() / episode_returns.len() as f64;

        let minibatch_size = config.minibatch_size.max(1).min(batch_states.len());
        for _ in 0..ppo_epochs {
            let mut indices: Vec<usize> = (0..batch_states.len()).collect();
            use rand::seq::SliceRandom;
            indices.shuffle(&mut rng);

            for chunk in indices.chunks(minibatch_size) {
                let mut actor_grads = Grads::zero(&actor.net);
                let mut critic_grads = Grads::zero(&critic.net);
                for &idx in chunk {
                    actor.accumulate_grad_ppo(
                        &batch_states[idx],
                        batch_actions[idx],
                        batch_log_probs[idx],
                        normalized_advantages[idx],
                        clip_epsilon,
                        current_entropy_coef,
                        &mut actor_grads,
                    );
                    critic.accumulate_grad(&batch_states[idx], batch_returns[idx], &mut critic_grads);
                }

                let n = chunk.len() as f64;
                for w in &mut actor_grads.layer_weights {
                    for x in w.iter_mut() { *x /= n; }
                }
                for b in &mut actor_grads.layer_biases {
                    for x in b.iter_mut() { *x /= n; }
                }
                macro_rules! scale { ($v:expr) => { for x in $v.iter_mut() { *x /= n; } }; }
                scale!(actor_grads.w_out); scale!(actor_grads.b_out);
                actor.net.add_regularization(&mut actor_grads, &config.regularization_type, config.regularization_lambda);
                actor_grads.clip_global_norm(config.grad_clip_norm);
                actor.net.apply_adam(&actor_grads, &mut actor_m, &mut actor_v, iteration + 1, current_actor_lr);

                for w in &mut critic_grads.layer_weights {
                    for x in w.iter_mut() { *x /= n; }
                }
                for b in &mut critic_grads.layer_biases {
                    for x in b.iter_mut() { *x /= n; }
                }
                scale!(critic_grads.w_out); scale!(critic_grads.b_out);
                critic.net.add_regularization(&mut critic_grads, &config.regularization_type, config.regularization_lambda);
                critic_grads.clip_global_norm(config.grad_clip_norm);
                critic.net.apply_adam(&critic_grads, &mut critic_m, &mut critic_v, iteration + 1, current_critic_lr);
            }
        }

        eprintln!("rl ppo: iteration {}/{} avg_return={:.4}", iteration, config.max_episodes, train_reward);

        let mut val_metric_opt: Option<f64> = None;
        if val_states.nrows() > 0 && ((iteration + 1) % validation_interval == 0 || iteration + 1 == config.max_episodes) {
            let val_metric = greedy_objective(&actor, &val_states, closes, val_indices, config, charges);
            val_metric_opt = Some(val_metric);
            if val_metric > best_val_metric + config.min_delta {
                best_val_metric = val_metric;
                best_actor = actor.clone();
                best_iteration = iteration + 1;
                iterations_since_best = 0;
            } else {
                iterations_since_best += validation_interval;
                if iterations_since_best >= config.early_stopping_patience {
                    eprintln!("rl ppo: early stopping at iteration {} best_iteration={} best_val_metric={:.4}", iteration + 1, best_iteration, best_val_metric);
                    break;
                }
            }
        }

        metrics.push(EpisodeMetric {
            episode: total_episodes,
            train_reward,
            val_metric: val_metric_opt,
        });
    }

    if best_iteration > 0 {
        actor = best_actor;
    }

    let train_pnl = evaluate(&actor, &train_states, closes, train_indices, config.allow_short, charges, config.position_deadband);
    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&actor, &val_states, closes, val_indices, config.allow_short, charges, config.position_deadband)
    } else { 0.0 };
    let test_pnl = if test_states.nrows() > 0 {
        evaluate(&actor, &test_states, closes, test_indices, config.allow_short, charges, config.position_deadband)
    } else { 0.0 };
    eprintln!("rl ppo: done. train_pnl={:.4} val_pnl={:.4} test_pnl={:.4}", train_pnl, val_pnl, test_pnl);
    let best_episode_actual = if best_iteration > 0 { best_iteration * batch_episodes } else { 0 };
    TrainResult { actor, final_train_reward: metrics.last().map(|m| m.train_reward).unwrap_or(0.0), train_pnl, val_pnl, test_pnl, episodes: total_episodes, best_episode: best_episode_actual, metrics }
}

pub fn evaluate(
    actor: &Actor,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    allow_short: bool,
    charges: &BrokerCharges,
    position_deadband: f64,
) -> f64 {
    let n = states.nrows();
    assert_eq!(n, candle_indices.len(), "state rows and candle indices must match");
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut total_pnl = 0.0f64;

    for t in 0..n {
        let state = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let state = decision_state(&state, position, holding, entry_price, price);
        let action = actor.greedy_action(&state, allow_short);

        if actor.continuous_action {
            let delta = action - position;
            if delta.abs() >= position_deadband {
                if position > 0.0 && action < position {
                    let closed = position - action.max(0.0);
                    total_pnl += closed * (price - entry_price) - charges.cost(entry_price, price, closed);
                } else if position < 0.0 && action > position {
                    let closed = position.abs() - action.min(0.0).abs();
                    total_pnl += closed * (entry_price - price) - charges.cost(price, entry_price, closed);
                }
                if action == 0.0 {
                    entry_price = 0.0;
                } else if position == 0.0 {
                    entry_price = price;
                } else if position.signum() != action.signum() {
                    entry_price = price;
                } else {
                    // Same direction
                    if action.abs() > position.abs() {
                        let same_dir_pos = if position > 0.0 { position.min(action) } else { position.max(action) };
                        let added = delta.abs();
                        let new_size = action.abs();
                        if new_size > 0.0 {
                            entry_price = (entry_price * same_dir_pos.abs() + price * added) / new_size;
                        }
                    }
                    // If reducing, entry_price stays the same for remaining shares
                }
                position = action;
            }
            holding = if position != 0.0 { holding + 1 } else { 0 };
        } else {
            match (position, action) {
                (0.0, 1.0) => { position = 1.0; entry_price = price; holding = 0; }
                (0.0, 2.0) if allow_short => { position = -1.0; entry_price = price; holding = 0; }
                (1.0, 0.0) | (1.0, 2.0) => {
                    total_pnl += close_position_reward(position, entry_price, price, charges);
                    position = 0.0; holding = 0;
                    if action == 2.0 && allow_short { position = -1.0; entry_price = price; }
                }
                (-1.0, 0.0) | (-1.0, 1.0) => {
                    total_pnl += close_position_reward(position, entry_price, price, charges);
                    position = 0.0; holding = 0;
                    if action == 1.0 { position = 1.0; entry_price = price; }
                }
                _ => {}
            }
            if position != 0.0 { holding += 1; }
        }
    }
    if position != 0.0 && n > 0 {
        let last = closes[candle_indices[n - 1]];
        total_pnl += close_position_reward(position, entry_price, last, charges);
    }
    total_pnl
}

pub fn weights_to_bytes(actor: &Actor) -> Result<Vec<u8>, String> {
    let params = actor.net.params();
    if params.iter().any(|v| !v.is_finite()) {
        return Err("training diverged: network weights contain NaN/Inf".to_string());
    }
    Ok(params.iter().flat_map(|&v| v.to_le_bytes()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mlp_forward() {
        let net = MLP::new(5, 4, 2, Activation::Tanh, 3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (pre, acts, logits) = net.forward(&x);
        assert_eq!(pre.len(), 2);
        assert_eq!(acts.len(), 2);
        assert_eq!(logits.len(), 3);
    }

    #[test]
    fn test_actor_forward_discrete() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (pre, acts, probs) = actor.forward_full(&x);
        assert_eq!(pre.len(), 2);
        assert_eq!(acts.len(), 2);
        assert!(probs.iter().all(|&p| p >= 0.0 && p <= 1.0));
        assert!((probs.iter().sum::<f64>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_actor_forward_continuous() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, true, 0.3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (_, _, probs) = actor.forward_full(&x);
        assert!(probs[0] >= -1.0 && probs[0] <= 1.0);
    }

    #[test]
    fn test_actor_grads_reinforce() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut grads = Grads::zero(&actor.net);
        actor.accumulate_grad(&x, 0.0, 1.0, &mut grads);

        for w in &grads.layer_weights {
            assert!(w.iter().any(|&v| v != 0.0), "layer weight gradient should be non-zero");
        }
        for b in &grads.layer_biases {
            assert!(b.iter().any(|&v| v != 0.0), "layer bias gradient should be non-zero");
        }
        assert!(grads.w_out.iter().any(|&v| v != 0.0), "w_out gradient should be non-zero");
        assert!(grads.b_out.iter().any(|&v| v != 0.0), "b_out gradient should be non-zero");
    }

    #[test]
    fn test_actor_grads_ppo() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut grads = Grads::zero(&actor.net);
        actor.accumulate_grad_ppo(&x, 0.0, -0.5, 1.0, 0.2, 0.01, &mut grads);

        for w in &grads.layer_weights {
            assert!(w.iter().any(|&v| v != 0.0), "layer weight gradient should be non-zero");
        }
        for b in &grads.layer_biases {
            assert!(b.iter().any(|&v| v != 0.0), "layer bias gradient should be non-zero");
        }
        assert!(grads.w_out.iter().any(|&v| v != 0.0), "w_out gradient should be non-zero");
    }

    #[test]
    fn test_critic_forward_and_grads() {
        let critic = Critic::new(5, 4, 2, Activation::Tanh);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let value = critic.forward_value(&x);
        assert!(value.is_finite());

        let mut grads = Grads::zero(&critic.net);
        critic.accumulate_grad(&x, 0.5, &mut grads);

        for w in &grads.layer_weights {
            assert!(w.iter().any(|&v| v != 0.0), "layer weight gradient should be non-zero");
        }
        for b in &grads.layer_biases {
            assert!(b.iter().any(|&v| v != 0.0), "layer bias gradient should be non-zero");
        }
        assert!(grads.w_out.iter().any(|&v| v != 0.0), "w_out gradient should be non-zero");
    }

    #[test]
    fn test_mlp_adam_update() {
        let mut actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut grads = Grads::zero(&actor.net);
        actor.accumulate_grad(&x, 0.0, 1.0, &mut grads);

        let mut m = AdamState::zero(&actor.net);
        let mut v = AdamState::zero(&actor.net);
        actor.net.apply_adam(&grads, &mut m, &mut v, 1, 1e-3);

        let params_after = actor.net.params();
        assert!(params_after.iter().any(|&v| v != 0.0));
    }

    #[test]
    fn test_mlp_params_roundtrip() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let params = actor.net.params();
        assert!(!params.is_empty());
        assert!(params.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_mlp_regularization() {
        let actor = Actor::new(5, 4, 2, Activation::Tanh, false, 0.3);
        let mut grads = Grads::zero(&actor.net);

        actor.net.add_regularization(&mut grads, "l2", 0.01);
        for (w, g) in actor.net.layer_weights.iter().zip(&grads.layer_weights) {
            let expected = w * 0.01;
            assert!(expected.iter().zip(g.iter()).all(|(e, a)| (e - a).abs() < 1e-12));
        }
        let expected_w_out = &actor.net.w_out * 0.01;
        assert!(expected_w_out.iter().zip(grads.w_out.iter()).all(|(e, a)| (e - a).abs() < 1e-12));
    }
}
