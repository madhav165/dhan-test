use ndarray::Array2;
use rand::Rng;

pub type Action = f64;

#[derive(Clone, Debug, PartialEq)]
pub enum Activation {
    Tanh,
    Relu,
}

#[derive(Clone)]
pub struct MLP {
    pub layers: Vec<(Vec<f64>, Vec<f64>)>, // (weight, bias) for each hidden layer
    pub w_out: Vec<f64>,
    pub b_out: Vec<f64>,
    pub w_value: Vec<f64>,
    pub b_value: f64,
    pub input_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub activation: Activation,
    pub continuous_action: bool,
    pub action_std: f64,
}

impl MLP {
    pub fn new(input_size: usize, hidden_size: usize, num_layers: usize, activation: Activation, continuous_action: bool, action_std: f64) -> Self {
        use rand_distr::{Normal, Distribution};
        let mut rng = rand::rng();
        let init_scale = match activation {
            Activation::Relu => (2.0 / input_size as f64).sqrt(),
            Activation::Tanh => (1.0 / input_size as f64).sqrt(),
        };
        let normal = Normal::new(0.0, init_scale).unwrap();
        let mut init = |n: usize| -> Vec<f64> { (0..n).map(|_| normal.sample(&mut rng)).collect() };
        
        let mut layers = vec![];
        for layer_idx in 0..num_layers {
            let prev_size = if layer_idx == 0 { input_size } else { hidden_size };
            let w = init(hidden_size * prev_size);
            let b = vec![0.0; hidden_size];
            layers.push((w, b));
        }
        
        let out_size = if continuous_action { 1 } else { 3 };
        Self {
            layers,
            w_out: init(out_size * hidden_size),
            b_out: vec![0.0; out_size],
            w_value: init(hidden_size),
            b_value: 0.0,
            input_size,
            hidden_size,
            num_layers,
            activation,
            continuous_action,
            action_std,
        }
    }

    fn matmul(w: &[f64], x: &[f64], rows: usize, cols: usize) -> Vec<f64> {
        (0..rows).map(|i| (0..cols).map(|j| w[i * cols + j] * x[j]).sum::<f64>()).collect()
    }

    fn activate(&self, v: f64) -> f64 {
        match self.activation {
            Activation::Tanh => v.tanh(),
            Activation::Relu => v.max(0.0),
        }
    }

    fn act_derivative(&self, v: f64) -> f64 {
        match self.activation {
            Activation::Tanh => 1.0 - v * v,
            Activation::Relu => if v > 0.0 { 1.0 } else { 0.0 },
        }
    }

    /// Forward pass. Returns (pre_activations, activations, output) for all layers.
    /// For discrete: output is [p0, p1, p2] softmax probs.
    /// For continuous: output is [mean, 0.0, 0.0] where mean is tanh-constrained.
    pub fn forward_full(&self, x: &[f64]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, [f64; 3]) {
        let mut pre_acts = vec![];
        let mut acts = vec![];
        let mut current = x.to_vec();
        
        for (w, b) in &self.layers {
            let pre: Vec<f64> = Self::matmul(w, &current, self.hidden_size, current.len())
                .iter().zip(b).map(|(v, b)| v + b).collect();
            let h: Vec<f64> = pre.iter().map(|&v| self.activate(v)).collect();
            pre_acts.push(pre);
            acts.push(h.clone());
            current = h;
        }
        
        if self.continuous_action {
            let logit = Self::matmul(&self.w_out, &current, 1, self.hidden_size)[0] + self.b_out[0];
            let mean = logit.tanh();
            return (pre_acts, acts, [mean, 0.0, 0.0]);
        }
        
        let logits: Vec<f64> = Self::matmul(&self.w_out, &current, 3, self.hidden_size)
            .iter().zip(&self.b_out).map(|(v, b)| v + b).collect();
        
        let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp: Vec<f64> = logits.iter().map(|v| (v - max).exp()).collect();
        let sum: f64 = exp.iter().sum();
        let probs = [exp[0] / sum, exp[1] / sum, exp[2] / sum];
        
        (pre_acts, acts, probs)
    }

    pub fn forward_full_with_value(&self, x: &[f64]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, [f64; 3], f64) {
        let (pre_acts, acts, probs) = self.forward_full(x);
        let h_last = acts.last().unwrap();
        let value = h_last.iter().zip(&self.w_value).map(|(h, w)| h * w).sum::<f64>() + self.b_value;
        (pre_acts, acts, probs, value)
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
            // d(log π)/d(mean) = (action - mean) / std^2
            // With tanh on mean: d(mean)/d(logit) = 1 - mean^2
            let d_mean = -advantage * (action - mean) / (std * std) * (1.0 - mean * mean);
            for j in 0..self.hidden_size {
                grads.w_out[j] += d_mean * h_last[j];
            }
            grads.b_out[0] += d_mean;
            
            let mut d_h = vec![0.0f64; self.hidden_size];
            for j in 0..self.hidden_size {
                d_h[j] += self.w_out[j] * d_mean;
            }
            
            for layer_idx in (0..self.num_layers).rev() {
                let pre = &pre_acts[layer_idx];
                let prev_h = if layer_idx == 0 { x.to_vec() } else { acts[layer_idx - 1].clone() };
                let d_pre: Vec<f64> = pre.iter().zip(&d_h).map(|(&p, &g)| g * self.act_derivative(p)).collect();
                
                for i in 0..self.hidden_size {
                    for j in 0..prev_h.len() {
                        grads.layer_weights[layer_idx][i * prev_h.len() + j] += d_pre[i] * prev_h[j];
                    }
                    grads.layer_biases[layer_idx][i] += d_pre[i];
                }
                
                if layer_idx > 0 {
                    d_h = vec![0.0f64; prev_h.len()];
                    let (w, _) = &self.layers[layer_idx];
                    for j in 0..prev_h.len() {
                        for i in 0..self.hidden_size {
                            d_h[j] += w[i * prev_h.len() + j] * d_pre[i];
                        }
                    }
                }
            }
            return;
        }
        
        let mut d_logits = [0.0f64; 3];
        for k in 0..3 {
            let indicator = if k == action as usize { 1.0 } else { 0.0 };
            d_logits[k] = -advantage * (indicator - probs[k]);
        }
        
        for i in 0..3 {
            for j in 0..self.hidden_size {
                grads.w_out[i * self.hidden_size + j] += d_logits[i] * h_last[j];
            }
            grads.b_out[i] += d_logits[i];
        }
        
        let mut d_h = vec![0.0f64; self.hidden_size];
        for j in 0..self.hidden_size {
            for i in 0..3 {
                d_h[j] += self.w_out[i * self.hidden_size + j] * d_logits[i];
            }
        }
        
        for layer_idx in (0..self.num_layers).rev() {
            let pre = &pre_acts[layer_idx];
            let prev_h = if layer_idx == 0 { x.to_vec() } else { acts[layer_idx - 1].clone() };
            let d_pre: Vec<f64> = pre.iter().zip(&d_h).map(|(&p, &g)| g * self.act_derivative(p)).collect();
            
            for i in 0..self.hidden_size {
                for j in 0..prev_h.len() {
                    grads.layer_weights[layer_idx][i * prev_h.len() + j] += d_pre[i] * prev_h[j];
                }
                grads.layer_biases[layer_idx][i] += d_pre[i];
            }
            
            if layer_idx > 0 {
                d_h = vec![0.0f64; prev_h.len()];
                let (w, _) = &self.layers[layer_idx];
                for j in 0..prev_h.len() {
                    for i in 0..self.hidden_size {
                        d_h[j] += w[i * prev_h.len() + j] * d_pre[i];
                    }
                }
            }
        }
    }

    /// Accumulate PPO gradient for a single transition.
    pub fn accumulate_grad_ppo(
        &self,
        x: &[f64],
        action: Action,
        old_log_prob: f64,
        advantage: f64,
        ret: f64,
        clip_epsilon: f64,
        value_coef: f64,
        entropy_coef: f64,
        grads: &mut Grads,
    ) {
        let (pre_acts, acts, probs, value) = self.forward_full_with_value(x);
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
                // d(ratio)/d(mean) = ratio * d(log π)/d(mean) = ratio * (action - mean) / std^2
                d_mean = -advantage * ratio * diff / (std * std);
            }
            // Entropy of Gaussian is constant w.r.t. mean, so no entropy gradient for mean.
            // Backprop through tanh on mean.
            d_mean *= 1.0 - mean * mean;

            let d_value = value_coef * (value - ret);

            for j in 0..self.hidden_size {
                grads.w_out[j] += d_mean * h_last[j];
            }
            grads.b_out[0] += d_mean;

            for j in 0..self.hidden_size {
                grads.w_value[j] += d_value * h_last[j];
            }
            grads.b_value += d_value;

            let mut d_h = vec![0.0f64; self.hidden_size];
            for j in 0..self.hidden_size {
                d_h[j] += self.w_out[j] * d_mean;
                d_h[j] += self.w_value[j] * d_value;
            }

            for layer_idx in (0..self.num_layers).rev() {
                let pre = &pre_acts[layer_idx];
                let prev_h = if layer_idx == 0 { x.to_vec() } else { acts[layer_idx - 1].clone() };
                let d_pre: Vec<f64> = pre.iter().zip(&d_h).map(|(&p, &g)| g * self.act_derivative(p)).collect();
                
                for i in 0..self.hidden_size {
                    for j in 0..prev_h.len() {
                        grads.layer_weights[layer_idx][i * prev_h.len() + j] += d_pre[i] * prev_h[j];
                    }
                    grads.layer_biases[layer_idx][i] += d_pre[i];
                }
                
                if layer_idx > 0 {
                    d_h = vec![0.0f64; prev_h.len()];
                    let (w, _) = &self.layers[layer_idx];
                    for j in 0..prev_h.len() {
                        for i in 0..self.hidden_size {
                            d_h[j] += w[i * prev_h.len() + j] * d_pre[i];
                        }
                    }
                }
            }
            return;
        }

        let action_idx = action as usize;
        let new_log_prob = probs[action_idx].max(1e-12).ln();
        let ratio = (new_log_prob - old_log_prob).exp();

        let clipped = ratio.clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon);
        let use_clipped = ratio * advantage > clipped * advantage;

        let mut d_logits = [0.0f64; 3];
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

        let d_value = value_coef * (value - ret);

        for i in 0..3 {
            for j in 0..self.hidden_size {
                grads.w_out[i * self.hidden_size + j] += d_logits[i] * h_last[j];
            }
            grads.b_out[i] += d_logits[i];
        }

        for j in 0..self.hidden_size {
            grads.w_value[j] += d_value * h_last[j];
        }
        grads.b_value += d_value;

        let mut d_h = vec![0.0f64; self.hidden_size];
        for j in 0..self.hidden_size {
            for i in 0..3 {
                d_h[j] += self.w_out[i * self.hidden_size + j] * d_logits[i];
            }
            d_h[j] += self.w_value[j] * d_value;
        }

        for layer_idx in (0..self.num_layers).rev() {
            let pre = &pre_acts[layer_idx];
            let prev_h = if layer_idx == 0 { x.to_vec() } else { acts[layer_idx - 1].clone() };
            let d_pre: Vec<f64> = pre.iter().zip(&d_h).map(|(&p, &g)| g * self.act_derivative(p)).collect();
            
            for i in 0..self.hidden_size {
                for j in 0..prev_h.len() {
                    grads.layer_weights[layer_idx][i * prev_h.len() + j] += d_pre[i] * prev_h[j];
                }
                grads.layer_biases[layer_idx][i] += d_pre[i];
            }
            
            if layer_idx > 0 {
                d_h = vec![0.0f64; prev_h.len()];
                let (w, _) = &self.layers[layer_idx];
                for j in 0..prev_h.len() {
                    for i in 0..self.hidden_size {
                        d_h[j] += w[i * prev_h.len() + j] * d_pre[i];
                    }
                }
            }
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
        
        for i in 0..self.layers.len() {
            update!(self.layers[i].0, grads.layer_weights[i], m.layer_weights[i], v.layer_weights[i]);
            update!(self.layers[i].1, grads.layer_biases[i], m.layer_biases[i], v.layer_biases[i]);
        }
        update!(self.w_out, grads.w_out, m.w_out, v.w_out);
        update!(self.b_out, grads.b_out, m.b_out, v.b_out);
        update!(self.w_value, grads.w_value, m.w_value, v.w_value);
        m.b_value = beta1 * m.b_value + (1.0 - beta1) * grads.b_value;
        let v_bv = beta2 * v.b_value + (1.0 - beta2) * grads.b_value.powi(2);
        v.b_value = v_bv;
        self.b_value -= lr * (m.b_value / bc1) / ((v_bv / bc2).sqrt() + eps);
    }

    pub fn params(&self) -> Vec<f64> {
        let mut p = vec![];
        for (w, b) in &self.layers {
            p.extend_from_slice(w);
            p.extend_from_slice(b);
        }
        p.extend_from_slice(&self.w_out);
        p.extend_from_slice(&self.b_out);
        p.extend_from_slice(&self.w_value);
        p.push(self.b_value);
        p
    }

    pub fn add_regularization(&self, grads: &mut Grads, reg_type: &str, lambda: f64) {
        if lambda <= 0.0 || reg_type == "none" {
            return;
        }
        match reg_type {
            "l1" => {
                for (layer_idx, (w, _)) in self.layers.iter().enumerate() {
                    for j in 0..w.len() {
                        grads.layer_weights[layer_idx][j] += lambda * w[j].signum();
                    }
                }
                for j in 0..self.w_out.len() {
                    grads.w_out[j] += lambda * self.w_out[j].signum();
                }
                for j in 0..self.w_value.len() {
                    grads.w_value[j] += lambda * self.w_value[j].signum();
                }
            }
            "l2" => {
                for (layer_idx, (w, _)) in self.layers.iter().enumerate() {
                    for j in 0..w.len() {
                        grads.layer_weights[layer_idx][j] += lambda * w[j];
                    }
                }
                for j in 0..self.w_out.len() {
                    grads.w_out[j] += lambda * self.w_out[j];
                }
                for j in 0..self.w_value.len() {
                    grads.w_value[j] += lambda * self.w_value[j];
                }
            }
            _ => {}
        }
    }
}

pub struct Grads {
    pub layer_weights: Vec<Vec<f64>>,
    pub layer_biases: Vec<Vec<f64>>,
    pub w_out: Vec<f64>,
    pub b_out: Vec<f64>,
    pub w_value: Vec<f64>,
    pub b_value: f64,
}

impl Grads {
    fn zero(net: &MLP) -> Self {
        Self {
            layer_weights: net.layers.iter().map(|(w, _)| vec![0.0; w.len()]).collect(),
            layer_biases: net.layers.iter().map(|(_, b)| vec![0.0; b.len()]).collect(),
            w_out: vec![0.0; net.w_out.len()],
            b_out: vec![0.0; net.b_out.len()],
            w_value: vec![0.0; net.w_value.len()],
            b_value: 0.0,
        }
    }

    fn clip_global_norm(&mut self, max_norm: f64) {
        let sum_sq = self.layer_weights.iter().flat_map(|w| w.iter())
            .chain(self.layer_biases.iter().flat_map(|b| b.iter()))
            .chain(&self.w_out).chain(&self.b_out)
            .chain(&self.w_value).chain(std::iter::once(&self.b_value))
            .map(|v| v * v)
            .sum::<f64>();
        let norm = sum_sq.sqrt();
        if norm <= max_norm || norm < 1e-12 {
            return;
        }
        let scale = max_norm / norm;
        for w in &mut self.layer_weights {
            for v in w.iter_mut() { *v *= scale; }
        }
        for b in &mut self.layer_biases {
            for v in b.iter_mut() { *v *= scale; }
        }
        for v in self.w_out.iter_mut().chain(&mut self.b_out).chain(&mut self.w_value) {
            *v *= scale;
        }
        self.b_value *= scale;
    }
}

pub struct AdamState {
    pub layer_weights: Vec<Vec<f64>>,
    pub layer_biases: Vec<Vec<f64>>,
    pub w_out: Vec<f64>,
    pub b_out: Vec<f64>,
    pub w_value: Vec<f64>,
    pub b_value: f64,
}

impl AdamState {
    fn zero(net: &MLP) -> Self {
        Self {
            layer_weights: net.layers.iter().map(|(w, _)| vec![0.0; w.len()]).collect(),
            layer_biases: net.layers.iter().map(|(_, b)| vec![0.0; b.len()]).collect(),
            w_out: vec![0.0; net.w_out.len()],
            b_out: vec![0.0; net.b_out.len()],
            w_value: vec![0.0; net.w_value.len()],
            b_value: 0.0,
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
    pub training_method: String,
    pub ppo_epochs: usize,
    pub clip_epsilon: f64,
    pub value_coef: f64,
    pub entropy_coef: f64,
    pub gae_lambda: f64,
    pub batch_episodes: usize,
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
    pub action_penalty: f64,
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
            gamma: 0.99,
            allow_short: false,
            reward_type: "pnl".into(),
            penalty_holding_days: None,
            max_holding_days: None,
            penalty_trades_per_month: None,
            max_trades_per_month: None,
            training_method: "ppo".into(),
            ppo_epochs: 4,
            clip_epsilon: 0.2,
            value_coef: 0.5,
            entropy_coef: 0.01,
            gae_lambda: 0.95,
            batch_episodes: 8,
            hidden_size: 64,
            num_layers: 2,
            activation: "relu".into(),
            reward_norm: true,
            lr_schedule: true,
            entropy_anneal: true,
            regularization_type: "none".into(),
            regularization_lambda: 0.0,
            continuous_action: false,
            action_std: 0.3,
            action_penalty: 0.0,
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

fn normalize_rewards(rewards: &mut [f64]) {
    if rewards.len() < 2 { return; }
    let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
    let std = (rewards.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rewards.len() as f64).sqrt().max(1e-8);
    for r in rewards.iter_mut() {
        *r = (*r - mean) / std;
    }
}

fn lr_at_step(initial_lr: f64, step: usize, total_steps: usize) -> f64 {
    if total_steps == 0 { return initial_lr; }
    let frac = step as f64 / total_steps as f64;
    initial_lr * (1.0 - frac)
}

fn entropy_at_step(initial_entropy: f64, step: usize, total_steps: usize) -> f64 {
    if total_steps == 0 { return initial_entropy; }
    let frac = step as f64 / total_steps as f64;
    initial_entropy * (1.0 - frac)
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

        if delta.abs() > 1e-8 {
            // Close/reduced portion: only subtract costs (MTM already captured the PnL)
            if prev_pos > 0.0 && action < prev_pos {
                let closed = prev_pos - action.max(0.0);
                reward -= charges.cost(*entry_price, price) * closed;
                *trades += 1;
            } else if prev_pos < 0.0 && action > prev_pos {
                let closed = prev_pos.abs() - action.min(0.0).abs();
                reward -= charges.cost(price, *entry_price) * closed;
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
                // Same direction: update weighted average
                let same_dir_pos = if prev_pos > 0.0 { prev_pos.min(action) } else { prev_pos.max(action) };
                let added = delta.abs();
                let new_size = action.abs();
                if new_size > 0.0 {
                    *entry_price = (*entry_price * same_dir_pos.abs() + price * added) / new_size;
                }
            }
        }

        *position = action;
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
                reward -= charges.cost(*entry_price, price);
                *trades += 1;
                *position = 0.0; *holding = 0;
                if action == 2.0 && config.allow_short {
                    *position = -1.0; *entry_price = price; *trades += 1;
                }
            }
            (-1.0, 0.0) | (-1.0, 1.0) => {
                // MTM already captured the PnL; only subtract costs
                reward -= charges.cost(price, *entry_price);
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
    if position > 0.0 {
        position * (price - entry_price) - charges.cost(entry_price, price) * position
    } else if position < 0.0 {
        position.abs() * (entry_price - price) - charges.cost(price, entry_price) * position.abs()
    } else {
        0.0
    }
}

fn greedy_objective(
    net: &MLP,
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
        let action = net.greedy_action(&state, config.allow_short);
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

        let action = net.greedy_action(
            &decision_state(&base, position, holding, entry_price, price),
            allow_short,
        );

        if net.continuous_action {
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
    net: &MLP,
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
        let (action, _) = net.sample_action(&state, rng, config.allow_short);
        let prev_price = if t > 0 { closes[candle_indices[row_idx - 1]] } else { price };
        let r = step_reward(action, prev_action, &mut position, &mut entry_price, &mut holding, &mut trades, price, prev_price, config, charges);
        steps.push(Step { state, action, ret: 0.0 });
        rewards.push(r);
        prev_action = action;
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

fn gae(rewards: &[f64], values: &[f64], gamma: f64, lambda: f64) -> Vec<f64> {
    let n = rewards.len();
    let mut advantages = vec![0.0; n];
    let mut gae = 0.0;
    for t in (0..n).rev() {
        let next_value = if t + 1 < n { values[t + 1] } else { 0.0 };
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
    net: &MLP,
    states: &Array2<f64>,
    closes: &[f64],
    candle_indices: &[usize],
    config: &TrainConfig,
    charges: &BrokerCharges,
    rng: &mut impl Rng,
) -> (Vec<TrajectoryStep>, f64) {
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
        let (action, log_prob) = net.sample_action(&state, rng, config.allow_short);
        let (_, _, _, value) = net.forward_full_with_value(&state);
        let prev_price = if t > 0 { closes[candle_indices[row_idx - 1]] } else { price };
        let r = step_reward(action, prev_action, &mut position, &mut entry_price, &mut holding, &mut trades, price, prev_price, config, charges);
        steps.push(TrajectoryStep { state, action, log_prob, value, reward: r });
        rewards.push(r);
        prev_action = action;
    }

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
    (steps, episode_return)
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
    let mut net = MLP::new(input_size, hidden_size, config.num_layers, activation, config.continuous_action, config.action_std);
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
        let n_steps = steps.len() as f64;
        for w in &mut grads.layer_weights {
            for x in w.iter_mut() { *x /= n_steps; }
        }
        for b in &mut grads.layer_biases {
            for x in b.iter_mut() { *x /= n_steps; }
        }
        macro_rules! norm { ($v:expr) => { for x in $v.iter_mut() { *x /= n_steps; } }; }
        norm!(grads.w_out); norm!(grads.b_out);
        net.add_regularization(&mut grads, &config.regularization_type, config.regularization_lambda);
        grads.clip_global_norm(config.grad_clip_norm);

        let lr = if config.lr_schedule {
            lr_at_step(config.lr, ep, config.max_episodes)
        } else {
            config.lr
        };
        net.apply_adam(&grads, &mut m, &mut v, ep + 1, lr);

        eprintln!("rl reinforce: episode {}/{} return={:.4}", ep, config.max_episodes, final_train_reward);

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
        net = best_net;
    }

    let train_pnl = evaluate(&net, &train_states, closes, train_indices, config.allow_short, charges);
    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&net, &val_states, closes, val_indices, config.allow_short, charges)
    } else { 0.0 };
    let test_pnl = if test_states.nrows() > 0 {
        evaluate(&net, &test_states, closes, test_indices, config.allow_short, charges)
    } else { 0.0 };
    eprintln!("rl reinforce: done. train_pnl={:.4} val_pnl={:.4} test_pnl={:.4}", train_pnl, val_pnl, test_pnl);
    TrainResult { net, final_train_reward, train_pnl, val_pnl, test_pnl, episodes: episodes_run, best_episode, metrics }
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
    let mut net = MLP::new(input_size, hidden_size, config.num_layers, activation, config.continuous_action, config.action_std);
    let mut m = AdamState::zero(&net);
    let mut v = AdamState::zero(&net);
    let mut rng = rand::rng();
    let mut best_net = net.clone();
    let mut best_val_metric = f64::NEG_INFINITY;
    let mut best_iteration = 0usize;
    let mut iterations_since_best = 0usize;
    let mut total_episodes = 0usize;
    let mut metrics: Vec<EpisodeMetric> = vec![];
    let validation_interval = config.validation_interval.max(1);
    let ppo_epochs = config.ppo_epochs.max(1);
    let clip_epsilon = config.clip_epsilon;
    let value_coef = config.value_coef;
    let gae_lambda = config.gae_lambda;
    let batch_episodes = config.batch_episodes.max(1);

    for iteration in 0..config.max_episodes {
        let current_entropy_coef = if config.entropy_anneal {
            entropy_at_step(config.entropy_coef, iteration, config.max_episodes)
        } else {
            config.entropy_coef
        };
        let current_lr = if config.lr_schedule {
            lr_at_step(config.lr, iteration, config.max_episodes)
        } else {
            config.lr
        };

        let mut batch_states: Vec<Vec<f64>> = vec![];
        let mut batch_actions: Vec<Action> = vec![];
        let mut batch_log_probs: Vec<f64> = vec![];
        let mut batch_advantages: Vec<f64> = vec![];
        let mut batch_returns: Vec<f64> = vec![];
        let mut episode_returns = vec![];

        for _ in 0..batch_episodes {
            let (traj, ep_return) = rollout_ppo(&net, &train_states, closes, train_indices, config, charges, &mut rng);
            episode_returns.push(ep_return);
            total_episodes += 1;

            let traj_rewards: Vec<f64> = traj.iter().map(|t| t.reward).collect();
            let traj_values: Vec<f64> = traj.iter().map(|t| t.value).collect();
            let advantages = gae(&traj_rewards, &traj_values, config.gamma, gae_lambda);
            let returns: Vec<f64> = advantages.iter().zip(&traj_values).map(|(a, v)| a + v).collect();

            for (t, (adv, ret)) in traj.into_iter().zip(advantages.into_iter().zip(returns.into_iter())) {
                batch_states.push(t.state);
                batch_actions.push(t.action);
                batch_log_probs.push(t.log_prob);
                batch_advantages.push(adv);
                batch_returns.push(ret);
            }
        }

        if config.reward_norm {
            normalize_rewards(&mut batch_returns);
        }

        let adv_mean = batch_advantages.iter().sum::<f64>() / batch_advantages.len() as f64;
        let adv_std = (batch_advantages.iter().map(|a| (a - adv_mean).powi(2)).sum::<f64>() / batch_advantages.len() as f64).sqrt().max(1e-8);
        let normalized_advantages: Vec<f64> = batch_advantages.iter().map(|a| (a - adv_mean) / adv_std).collect();

        let train_reward = episode_returns.iter().sum::<f64>() / episode_returns.len() as f64;

        let minibatch_size = (batch_states.len() / 2).max(1);
        for _ in 0..ppo_epochs {
            let mut indices: Vec<usize> = (0..batch_states.len()).collect();
            use rand::seq::SliceRandom;
            indices.shuffle(&mut rng);

            for chunk in indices.chunks(minibatch_size) {
                let mut grads = Grads::zero(&net);
                for &idx in chunk {
                    net.accumulate_grad_ppo(
                        &batch_states[idx],
                        batch_actions[idx],
                        batch_log_probs[idx],
                        normalized_advantages[idx],
                        batch_returns[idx],
                        clip_epsilon,
                        value_coef,
                        current_entropy_coef,
                        &mut grads,
                    );
                }

                let n = chunk.len() as f64;
                for w in &mut grads.layer_weights {
                    for x in w.iter_mut() { *x /= n; }
                }
                for b in &mut grads.layer_biases {
                    for x in b.iter_mut() { *x /= n; }
                }
                macro_rules! scale { ($v:expr) => { for x in $v.iter_mut() { *x /= n; } }; }
                scale!(grads.w_out); scale!(grads.b_out); scale!(grads.w_value);
                grads.b_value /= n;
                net.add_regularization(&mut grads, &config.regularization_type, config.regularization_lambda);
                grads.clip_global_norm(config.grad_clip_norm);

                net.apply_adam(&grads, &mut m, &mut v, iteration + 1, current_lr);
            }
        }

        eprintln!("rl ppo: iteration {}/{} avg_return={:.4}", iteration, config.max_episodes, train_reward);

        let mut val_metric_opt: Option<f64> = None;
        if val_states.nrows() > 0 && ((iteration + 1) % validation_interval == 0 || iteration + 1 == config.max_episodes) {
            let val_metric = greedy_objective(&net, &val_states, closes, val_indices, config, charges);
            val_metric_opt = Some(val_metric);
            if val_metric > best_val_metric + config.min_delta {
                best_val_metric = val_metric;
                best_net = net.clone();
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
        net = best_net;
    }

    let train_pnl = evaluate(&net, &train_states, closes, train_indices, config.allow_short, charges);
    let val_pnl = if val_states.nrows() > 0 {
        evaluate(&net, &val_states, closes, val_indices, config.allow_short, charges)
    } else { 0.0 };
    let test_pnl = if test_states.nrows() > 0 {
        evaluate(&net, &test_states, closes, test_indices, config.allow_short, charges)
    } else { 0.0 };
    eprintln!("rl ppo: done. train_pnl={:.4} val_pnl={:.4} test_pnl={:.4}", train_pnl, val_pnl, test_pnl);
    let best_episode_actual = if best_iteration > 0 { best_iteration * batch_episodes } else { 0 };
    TrainResult { net, final_train_reward: metrics.last().map(|m| m.train_reward).unwrap_or(0.0), train_pnl, val_pnl, test_pnl, episodes: total_episodes, best_episode: best_episode_actual, metrics }
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
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0f64;
    let mut holding = 0usize;
    let mut total_pnl = 0.0f64;

    for t in 0..n {
        let state = states.row(t).to_vec();
        let price = closes[candle_indices[t]];
        let state = decision_state(&state, position, holding, entry_price, price);
        let action = net.greedy_action(&state, allow_short);

        if net.continuous_action {
            let delta = action - position;
            if delta.abs() > 1e-8 {
                if position > 0.0 && action < position {
                    let closed = position - action.max(0.0);
                    total_pnl += closed * (price - entry_price) - charges.cost(entry_price, price) * closed;
                } else if position < 0.0 && action > position {
                    let closed = position.abs() - action.min(0.0).abs();
                    total_pnl += closed * (entry_price - price) - charges.cost(price, entry_price) * closed;
                }
                if action == 0.0 {
                    entry_price = 0.0;
                } else if position == 0.0 {
                    entry_price = price;
                } else if position.signum() != action.signum() {
                    entry_price = price;
                } else {
                    let same_dir_pos = if position > 0.0 { position.min(action) } else { position.max(action) };
                    let added = delta.abs();
                    let new_size = action.abs();
                    if new_size > 0.0 {
                        entry_price = (entry_price * same_dir_pos.abs() + price * added) / new_size;
                    }
                }
            }
            position = action;
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

pub fn weights_to_bytes(net: &MLP) -> Result<Vec<u8>, String> {
    let params = net.params();
    if params.iter().any(|v| !v.is_finite()) {
        return Err("training diverged: network weights contain NaN/Inf".to_string());
    }
    Ok(params.iter().flat_map(|&v| v.to_le_bytes()).collect())
}
