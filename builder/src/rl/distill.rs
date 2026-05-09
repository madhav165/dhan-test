use std::collections::BTreeSet;
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
                    "{}if {} ≤ {:.4}:\n{}else:\n{}",
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

fn collect_features(node: &TreeNode, used: &mut BTreeSet<usize>) {
    if let TreeNode::Split { feature, left, right, .. } = node {
        used.insert(*feature);
        collect_features(left, used);
        collect_features(right, used);
    }
}

fn tree_to_rust(node: &TreeNode, depth: usize) -> String {
    let indent = "    ".repeat(depth + 1);
    match node {
        TreeNode::Leaf { action_mean } => {
            let sig: u8 = if *action_mean > 0.5 { 1 } else if *action_mean < -0.5 { 2 } else { 0 };
            format!("{}{}u8", indent, sig)
        }
        TreeNode::Split { feature, threshold, left, right } => {
            format!(
                "{}if f{} <= {:.10}_f64 {{\n{}\n{}}} else {{\n{}\n{}}}",
                indent, feature, threshold,
                tree_to_rust(left, depth + 1),
                indent,
                tree_to_rust(right, depth + 1),
                indent,
            )
        }
    }
}

fn indicator_vec_expr(spec: &IndicatorSpec) -> String {
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

pub fn distil_to_rust(
    net: &MLP,
    states: &Array2<f64>,
    feature_names: &[String],
    indicator_specs: &[IndicatorSpec],
    lookback: usize,
    means: &[f64],
    stds: &[f64],
    max_depth: usize,
) -> String {
    let n = states.nrows();
    if n == 0 { return "0u8".into(); }

    let state_vecs: Vec<Vec<f64>> = (0..n).map(|i| states.row(i).to_vec()).collect();
    let scores: Vec<f64> = state_vecs.iter().map(|s| dominant_logit(net, s)).collect();
    let tree = build_tree(&state_vecs, &scores, 0, max_depth);

    let mut used = BTreeSet::new();
    collect_features(&tree, &mut used);

    let num_inds = indicator_specs.len();
    let mut lines: Vec<String> = vec![];

    lines.push(format!("    if i < {} {{ return 0; }}", lookback));

    for &idx in &used {
        let mean = means.get(idx).copied().unwrap_or(0.0);
        let std = stds.get(idx).copied().unwrap_or(1.0);
        if idx < num_inds {
            let expr = indicator_vec_expr(&indicator_specs[idx]);
            lines.push(format!(
                "    let f{} = {{ let v = {}; if v[i].is_nan() {{ return 0; }} (v[i] - {:.10}_f64) / {:.10}_f64 }};",
                idx, expr, mean, std
            ));
        } else {
            let rel = idx - num_inds;
            let lag = rel / 5 + 1;
            let col = rel % 5;
            let raw = match col {
                0 | 3 => format!("prices[i - {}]", lag),
                1 => format!("highs[i - {}]", lag),
                2 => format!("lows[i - {}]", lag),
                4 => format!("volumes[i - {}]", lag),
                _ => "0.0".into(),
            };
            lines.push(format!(
                "    let f{} = ({} - {:.10}_f64) / {:.10}_f64;",
                idx, raw, mean, std
            ));
        }
        let _ = feature_names; // used for human-readable distil only
    }

    lines.push(tree_to_rust(&tree, 0));
    lines.join("\n")
}

pub fn distil(net: &MLP, states: &Array2<f64>, feature_names: &[String], max_depth: usize) -> String {
    let n = states.nrows();
    if n == 0 { return "No data".into(); }

    let state_vecs: Vec<Vec<f64>> = (0..n).map(|i| states.row(i).to_vec()).collect();
    let scores: Vec<f64> = state_vecs.iter().map(|s| dominant_logit(net, s)).collect();

    let tree = build_tree(&state_vecs, &scores, 0, max_depth);
    tree.to_text(feature_names, 0)
}
