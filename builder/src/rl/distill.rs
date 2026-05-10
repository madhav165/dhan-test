use ndarray::Array2;
use crate::rl::train::MLP;
use crate::rl::features::IndicatorSpec;

fn dominant_logit(net: &MLP, x: &[f64]) -> f64 {
    let (_, _, probs) = net.forward_full(x);
    // buy prob minus hold prob as signal strength
    probs[1] - probs[0]
}

pub fn feature_importance(net: &MLP, states: &Array2<f64>) -> Vec<f64> {
    let n = states.nrows();
    let d = states.ncols();
    if n == 0 { return vec![0.0; d]; }

    let baseline: Vec<f64> = (0..n)
        .map(|i| { let s = states.row(i).to_vec(); dominant_logit(net, &s) })
        .collect();

    (0..d).map(|j| {
        let diff: f64 = (0..n).map(|i| {
            let mut s = states.row(i).to_vec();
            s[j] = 0.0;
            (baseline[i] - dominant_logit(net, &s)).abs()
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

fn indicator_expr(spec: &IndicatorSpec) -> String {
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

pub fn net_to_rust(
    net: &MLP,
    indicator_specs: &[IndicatorSpec],
    lookback: usize,
    means: &[f64],
    stds: &[f64],
    allow_short: bool,
) -> String {
    let num_inds = indicator_specs.len();
    let state_dim = num_inds + 5 * lookback + 3;
    let mut lines: Vec<String> = vec![];

    lines.push(format!("    if i < {} {{ return 0; }}", lookback));

    // Embed weights and normalization stats
    lines.push(format!("    let w1: &[f64] = {};", fmt_slice(&net.w1)));
    lines.push(format!("    let b1: &[f64] = {};", fmt_slice(&net.b1)));
    lines.push(format!("    let w2: &[f64] = {};", fmt_slice(&net.w2)));
    lines.push(format!("    let b2: &[f64] = {};", fmt_slice(&net.b2)));
    lines.push(format!("    let w_out: &[f64] = {};", fmt_slice(&net.w_out)));
    lines.push(format!("    let b_out: &[f64] = {};", fmt_slice(&net.b_out)));
    lines.push(format!("    let means: &[f64] = {};", fmt_slice(means)));
    lines.push(format!("    let stds: &[f64] = {};", fmt_slice(stds)));
    lines.push(format!("    let hidden = {};", net.hidden_size));
    lines.push(format!("    let input = {};", net.input_size));

    // Build and normalize feature vector — same ordering as build_state_matrix
    lines.push(format!("    let mut feat = vec![0.0_f64; {}];", state_dim));
    for (idx, spec) in indicator_specs.iter().enumerate() {
        let expr = indicator_expr(spec);
        lines.push(format!(
            "    feat[{idx}] = {{ let v = {expr}; if v[i].is_nan() {{ return 255; }} (v[i] - means[{idx}]) / stds[{idx}] }};"
        ));
    }
    lines.push(format!("    let mut off = {};", num_inds));
    lines.push(format!("    for lag in 1_usize..={} {{", lookback));
    lines.push("        let t = i - lag;".into());
    // Use close as a fallback for older WASM callers that do not provide opens.
    lines.push("        let open = if opens.len() > t { opens[t] } else { prices[t] };".into());
    lines.push("        let raw = [open, highs[t], lows[t], prices[t], volumes[t]];".into());
    lines.push("        for k in 0..5_usize { feat[off+k] = (raw[k] - means[off+k]) / stds[off+k]; }".into());
    lines.push("        off += 5;".into());
    lines.push("    }".into());
    lines.push("    feat[off] = norm_position;".into());
    lines.push("    feat[off + 1] = norm_holding;".into());
    lines.push("    feat[off + 2] = norm_unrealized;".into());

    // Forward pass: input -> tanh -> tanh -> softmax
    lines.push("    let mm = |w: &[f64], x: &[f64], r: usize, c: usize| -> Vec<f64> { (0..r).map(|i| (0..c).map(|j| w[i*c+j]*x[j]).sum::<f64>()).collect() };".into());
    lines.push("    let h1: Vec<f64> = mm(w1,&feat,hidden,input).into_iter().zip(b1).map(|(v,b)| (v+b).tanh()).collect();".into());
    lines.push("    let h2: Vec<f64> = mm(w2,&h1,hidden,hidden).into_iter().zip(b2).map(|(v,b)| (v+b).tanh()).collect();".into());
    lines.push("    let lo: Vec<f64> = mm(w_out,&h2,3,hidden).into_iter().zip(b_out).map(|(v,b)| v+b).collect();".into());
    lines.push("    let mx = lo.iter().cloned().fold(f64::NEG_INFINITY, f64::max);".into());
    lines.push("    let ex: Vec<f64> = lo.iter().map(|v| (v-mx).exp()).collect();".into());
    lines.push("    let s: f64 = ex.iter().sum();".into());
    lines.push("    let p = [ex[0]/s, ex[1]/s, ex[2]/s];".into());

    if allow_short {
        lines.push("    if p[1] >= p[0] && p[1] >= p[2] { 1u8 } else if p[2] >= p[0] { 2u8 } else { 0u8 }".into());
    } else {
        lines.push("    if p[1] >= p[0] { 1u8 } else { 0u8 }".into());
    }

    lines.join("\n")
}

pub fn distil(net: &MLP, states: &Array2<f64>, feature_names: &[String], max_depth: usize) -> String {
    let n = states.nrows();
    if n == 0 { return "No data".into(); }

    let state_vecs: Vec<Vec<f64>> = (0..n).map(|i| states.row(i).to_vec()).collect();
    let scores: Vec<f64> = state_vecs.iter().map(|s| dominant_logit(net, s)).collect();

    let tree = build_tree(&state_vecs, &scores, 0, max_depth);
    format!("Thresholds are shown in normalized feature space.\n{}", tree.to_text(feature_names, 0))
}
