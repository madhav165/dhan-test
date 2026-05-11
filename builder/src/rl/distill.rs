use ndarray::Array2;
use crate::rl::train::{Actor, Activation};
use crate::rl::features::IndicatorSpec;

/// Code generation descriptor for a transformed feature.
/// The `expr` is a Rust expression string that evaluates to the feature value at index `i`.
pub struct FeatureCodegen {
    #[allow(dead_code)]
    pub name: String,
    pub expr: String,
}

/// Generate Rust expressions for stationary-transformed features.
/// This MUST match the logic in `features::stationary_transform` exactly.
pub fn codegen_transforms(specs: &[IndicatorSpec]) -> Vec<FeatureCodegen> {
    use std::collections::HashMap;
    let mut out = vec![];

    // Group BB components by period
    let mut bb_groups: HashMap<usize, (Option<usize>, Option<usize>, Option<usize>)> = HashMap::new();
    for (idx, spec) in specs.iter().enumerate() {
        if let IndicatorSpec::Bb { component, period } = spec {
            let entry = bb_groups.entry(*period).or_insert((None, None, None));
            match component.as_str() {
                "upper" => entry.0 = Some(idx),
                "middle" => entry.1 = Some(idx),
                "lower" => entry.2 = Some(idx),
                _ => {}
            }
        }
    }

    for spec in specs {
        match spec {
            IndicatorSpec::Ema { period } => {
                out.push(FeatureCodegen {
                    name: format!("ema_{}_dist", period),
                    expr: format!("{{ let ema = ema(prices, {}); (prices[i] - ema[i]) / ema[i].max(1e-8) }}", period),
                });
            }
            IndicatorSpec::Rsi { period } => {
                out.push(FeatureCodegen {
                    name: format!("rsi_{}_centered", period),
                    expr: format!("{{ let rsi = rsi(prices, {}); (rsi[i] - 50.0) / 50.0 }}", period),
                });
            }
            IndicatorSpec::Bb { component: _, period } => {
                // Only process each BB period once, when we see upper
                if let Some((Some(_), Some(_), Some(_))) = bb_groups.get(period) {
                    // Generate bandwidth
                    out.push(FeatureCodegen {
                        name: format!("bb_{}_bandwidth", period),
                        expr: format!(
                            "{{ let (u, m, l) = bb(prices, {}); (u[i] - l[i]) / m[i].max(1e-8) }}",
                            period
                        ),
                    });
                    // Generate %B
                    out.push(FeatureCodegen {
                        name: format!("bb_{}_percent_b", period),
                        expr: format!(
                            "{{ let (u, m, l) = bb(prices, {}); let range = u[i] - l[i]; if range < 1e-8 {{ 0.5 }} else {{ (prices[i] - l[i]) / range }} }}",
                            period
                        ),
                    });
                    // Remove from map so we don't duplicate
                    bb_groups.remove(period);
                }
            }
            IndicatorSpec::Obv => {
                out.push(FeatureCodegen {
                    name: "obv_delta_norm".into(),
                    expr: "{ let obv = obv(prices, volumes); let delta = if i > 0 { obv[i] - obv[i-1] } else { 0.0 }; let window = 20usize; if i >= window { let mean: f64 = (1..=window).map(|k| obv[i-k+1] - obv[i-k]).sum::<f64>() / window as f64; let std = ((1..=window).map(|k| { let d = obv[i-k+1] - obv[i-k]; (d - mean).powi(2) }).sum::<f64>() / window as f64).sqrt().max(1e-8); (delta - mean) / std } else { f64::NAN } }".into(),
                });
            }
            _ => {
                // Passthrough for any other indicators (backward compatibility)
                let expr = indicator_expr_raw(spec);
                out.push(FeatureCodegen {
                    name: spec.name(),
                    expr: format!("{{ let v = {}; v[i] }}", expr),
                });
            }
        }
    }
    out
}

fn indicator_expr_raw(spec: &IndicatorSpec) -> String {
    match spec {
        IndicatorSpec::Rsi { period } => format!("rsi(prices, {})", period),
        IndicatorSpec::Sma { period } => format!("sma(prices, {})", period),
        IndicatorSpec::Ema { period } => format!("ema(prices, {})", period),
        IndicatorSpec::Wma { period } => format!("wma(prices, {})", period),
        IndicatorSpec::Vwap => "vwap(prices, volumes)".into(),
        IndicatorSpec::Macd { component, fast, slow, signal_period } => {
            let pat = match component.as_str() { "signal" => "(_, v, _)", "histogram" => "(_, _, v)", _ => "(v, _, _)" };
            format!("{{ let {} = macd(prices, {}, {}, {}); v }}", pat, fast, slow, signal_period)
        }
        IndicatorSpec::Bb { component, period } => {
            let pat = match component.as_str() { "middle" => "(_, v, _)", "lower" => "(_, _, v)", _ => "(v, _, _)" };
            format!("{{ let {} = bb(prices, {}); v }}", pat, period)
        }
        IndicatorSpec::Atr { period } => format!("atr(highs, lows, prices, {})", period),
        IndicatorSpec::Stoch { period } => format!("stoch(highs, lows, prices, {})", period),
        IndicatorSpec::Obv => "obv(prices, volumes)".into(),
        IndicatorSpec::Cci { period } => format!("cci(highs, lows, prices, {})", period),
    }
}

fn dominant_logit(actor: &Actor, x: &[f64]) -> f64 {
    let (_, _, probs) = actor.forward_full(x);
    if actor.continuous_action {
        probs[0] // mean
    } else {
        // buy prob minus hold prob as signal strength
        probs[1] - probs[0]
    }
}

pub fn feature_importance(actor: &Actor, states: &Array2<f64>) -> Vec<f64> {
    let n = states.nrows();
    let d = states.ncols();
    if n == 0 { return vec![0.0; d]; }

    let baseline: Vec<f64> = (0..n)
        .map(|i| { let s = states.row(i).to_vec(); dominant_logit(actor, &s) })
        .collect();

    (0..d).map(|j| {
        let diff: f64 = (0..n).map(|i| {
            let mut s = states.row(i).to_vec();
            s[j] = 0.0;
            (baseline[i] - dominant_logit(actor, &s)).abs()
        }).sum::<f64>() / n as f64;
        diff
    }).collect()
}

pub fn normalise_importance(imp: &[f64]) -> Vec<f64> {
    let total: f64 = imp.iter().sum();
    if total < 1e-10 { return vec![1.0 / imp.len() as f64; imp.len()]; }
    imp.iter().map(|v| v / total).collect()
}

enum TreeNode {
    Leaf { action_mean: f64 },
    Split { feature: usize, threshold: f64, left: Box<TreeNode>, right: Box<TreeNode> },
}

impl TreeNode {
    fn to_text(&self, feature_names: &[String], depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            Self::Leaf { action_mean } => {
                // action_mean here is the dominant logit (buy - hold); interpret as signal
                let signal = if *action_mean > 0.5 { "BUY" } else if *action_mean < -0.5 { "SELL" } else { "HOLD" };
                format!("{}→ {} (score={:.2})\n", indent, signal, action_mean)
            }
            Self::Split { feature, threshold, left, right } => {
                let fname = feature_names.get(*feature).map(|s| s.as_str()).unwrap_or("?");
                format!(
                    "{}if {} ≤ {:.4} (normalized):\n{}else:\n{}",
                    indent, fname, threshold,
                    left.to_text(feature_names, depth + 1),
                    right.to_text(feature_names, depth + 1),
                )
            }
        }
    }
}

fn best_split(states: &[Vec<f64>], actions: &[f64], feature: usize) -> (f64, f64) {
    let mut vals: Vec<f64> = states.iter().map(|s| s[feature]).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    vals.dedup();
    if vals.is_empty() { return (0.0, f64::MAX); }

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
        .map(|(s, &a)| (s.clone(), a)).unzip();
    let (right_s, right_a): (Vec<_>, Vec<_>) = states.iter().zip(actions.iter())
        .filter(|(s, _)| s[best_feat] > best_thresh)
        .map(|(s, &a)| (s.clone(), a)).unzip();

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

fn fmt_slice(v: &[f64]) -> String {
    let vals: Vec<String> = v.iter().map(|x| format!("{:.10}_f64", x)).collect();
    format!("&[{}]", vals.join(","))
}

fn fmt_array1(v: &ndarray::Array1<f64>) -> String {
    let vals: Vec<String> = v.iter().map(|x| format!("{:.10}_f64", x)).collect();
    format!("&[{}]", vals.join(","))
}

fn fmt_array2(v: &ndarray::Array2<f64>) -> String {
    let vals: Vec<String> = v.iter().map(|x| format!("{:.10}_f64", x)).collect();
    format!("&[{}]", vals.join(","))
}



pub fn net_to_rust(
    actor: &Actor,
    features: &[FeatureCodegen],
    lookback: usize,
    means: &[f64],
    stds: &[f64],
    allow_short: bool,
) -> String {
    let num_inds = features.len();
    let state_dim = num_inds + 5 * lookback + 3;
    let mut lines: Vec<String> = vec![];

    lines.push(format!("    if i < {} {{ return 0.0; }}", lookback));

    for (idx, (w, b)) in actor.net.layer_weights.iter().zip(&actor.net.layer_biases).enumerate() {
        lines.push(format!("    let w{}: &[f64] = {};", idx, fmt_array2(w)));
        lines.push(format!("    let b{}: &[f64] = {};", idx, fmt_array1(b)));
    }
    lines.push(format!("    let w_out: &[f64] = {};", fmt_array2(&actor.net.w_out)));
    lines.push(format!("    let b_out: &[f64] = {};", fmt_array1(&actor.net.b_out)));
    lines.push(format!("    let means: &[f64] = {};", fmt_slice(means)));
    lines.push(format!("    let stds: &[f64] = {};", fmt_slice(stds)));
    lines.push(format!("    let hidden = {};", actor.net.hidden_size));
    lines.push(format!("    let input = {};", actor.net.input_size));

    lines.push(format!("    let mut feat = vec![0.0_f64; {}];", state_dim));
    for (idx, feat) in features.iter().enumerate() {
        lines.push(format!(
            "    feat[{idx}] = {{ let v = {}; if v.is_nan() {{ return f64::NAN; }} (v - means[{idx}]) / stds[{idx}] }};",
            feat.expr
        ));
    }
    lines.push(format!("    let mut off = {};", num_inds));
    lines.push("    let current_close = prices[i].max(1e-8);".into());
    lines.push("    let current_vol = volumes[i].max(1e-8);".into());
    lines.push(format!("    for lag in 1_usize..={} {{", lookback));
    lines.push("        let t = i - lag;".into());
    lines.push("        let open = if opens.len() > t { opens[t] } else { prices[t] };".into());
    lines.push("        let raw = [".into());
    lines.push("            (open - current_close) / current_close,".into());
    lines.push("            (highs[t] - current_close) / current_close,".into());
    lines.push("            (lows[t] - current_close) / current_close,".into());
    lines.push("            (prices[t] - current_close) / current_close,".into());
    lines.push("            (volumes[t] - current_vol) / current_vol,".into());
    lines.push("        ];".into());
    lines.push("        for k in 0..5_usize { feat[off+k] = (raw[k] - means[off+k]) / stds[off+k]; }".into());
    lines.push("        off += 5;".into());
    lines.push("    }".into());
    lines.push("    feat[off] = norm_position;".into());
    lines.push("    feat[off + 1] = norm_holding;".into());
    lines.push("    feat[off + 2] = norm_unrealized;".into());

    let act = match actor.net.activation {
        Activation::Tanh => "tanh()",
        Activation::Relu => "max(0.0)",
    };

    lines.push("    let mm = |w: &[f64], x: &[f64], r: usize, c: usize| -> Vec<f64> { (0..r).map(|i| (0..c).map(|j| w[i*c+j]*x[j]).sum::<f64>()).collect() };".into());
    lines.push("    let mut h = feat;".into());
    for (idx, _) in actor.net.layer_weights.iter().enumerate() {
        let prev = if idx == 0 { "input" } else { "hidden" };
        lines.push(format!(
            "    h = mm(w{idx},&h,hidden,{prev}).into_iter().zip(b{idx}).map(|(v,b)| (v+b).{act}).collect();"
        ));
    }
    if actor.continuous_action {
        lines.push("    mm(w_out,&h,1,hidden).into_iter().zip(b_out).map(|(v,b)| (v+b).tanh()).next().unwrap()".into());
    } else {
        lines.push("    let lo: Vec<f64> = mm(w_out,&h,3,hidden).into_iter().zip(b_out).map(|(v,b)| v+b).collect();".into());
        lines.push("    let mx = lo.iter().cloned().fold(f64::NEG_INFINITY, f64::max);".into());
        lines.push("    let ex: Vec<f64> = lo.iter().map(|v| (v-mx).exp()).collect();".into());
        lines.push("    let s: f64 = ex.iter().sum();".into());
        lines.push("    let p = [ex[0]/s, ex[1]/s, ex[2]/s];".into());

        if allow_short {
            lines.push("    if p[1] >= p[0] && p[1] >= p[2] { 1.0 } else if p[2] >= p[0] { -1.0 } else { 0.0 }".into());
        } else {
            lines.push("    if p[1] >= p[0] { 1.0 } else { 0.0 }".into());
        }
    }

    lines.join("\n")
}

pub fn distil(actor: &Actor, states: &Array2<f64>, feature_names: &[String], max_depth: usize) -> String {
    let n = states.nrows();
    if n == 0 { return "No data".into(); }

    let state_vecs: Vec<Vec<f64>> = (0..n).map(|i| states.row(i).to_vec()).collect();
    let scores: Vec<f64> = state_vecs.iter().map(|s| dominant_logit(actor, s)).collect();

    let tree = build_tree(&state_vecs, &scores, 0, max_depth);
    format!("Thresholds are shown in normalized feature space.\n{}", tree.to_text(feature_names, 0))
}
