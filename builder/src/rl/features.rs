use ndarray::{Array1, Array2};

pub struct Candles {
    pub timestamps: Vec<i64>,
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub closes: Vec<f64>,
    pub volumes: Vec<f64>,
}

pub fn compute_day_boundaries(timestamps: &[i64]) -> Vec<bool> {
    let mut boundaries = vec![false; timestamps.len()];
    for i in 1..timestamps.len() {
        let prev_day = timestamps[i - 1] / 86400;
        let curr_day = timestamps[i] / 86400;
        if prev_day != curr_day {
            boundaries[i - 1] = true;
        }
    }
    let len = boundaries.len();
    if len > 0 {
        boundaries[len - 1] = true;
    }
    boundaries
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "name", rename_all = "lowercase")]
pub enum IndicatorSpec {
    Rsi { period: usize },
    Sma { period: usize },
    Ema { period: usize },
    Wma { period: usize },
    Vwap,
    Macd { component: String, fast: usize, slow: usize, signal_period: usize },
    Bb { component: String, period: usize },
    Atr { period: usize },
    Stoch { period: usize },
    Obv,
    Cci { period: usize },
}

impl IndicatorSpec {
    pub fn name(&self) -> String {
        match self {
            Self::Rsi { period } => format!("rsi_{}", period),
            Self::Sma { period } => format!("sma_{}", period),
            Self::Ema { period } => format!("ema_{}", period),
            Self::Wma { period } => format!("wma_{}", period),
            Self::Vwap => "vwap".into(),
            Self::Macd { component, fast, slow, signal_period } =>
                format!("macd_{}_{}_{}_{}",fast, slow, signal_period, component),
            Self::Bb { component, period } => format!("bb_{}_{}", period, component),
            Self::Atr { period } => format!("atr_{}", period),
            Self::Stoch { period } => format!("stoch_{}", period),
            Self::Obv => "obv".into(),
            Self::Cci { period } => format!("cci_{}", period),
        }
    }
}

pub fn compute_indicators(candles: &Candles, specs: &[IndicatorSpec]) -> Vec<(String, Vec<f64>)> {
    use indicators::{
        rsi::rsi, sma::sma, ema::ema, wma::wma, vwap::vwap, macd::macd, bb::bb,
        atr::atr, stoch::stoch, obv::obv, cci::cci,
    };
    let mut out = vec![];
    for spec in specs {
        let series: Vec<f64> = match spec {
            IndicatorSpec::Rsi { period } => rsi(&candles.closes, *period),
            IndicatorSpec::Sma { period } => sma(&candles.closes, *period),
            IndicatorSpec::Ema { period } => ema(&candles.closes, *period),
            IndicatorSpec::Wma { period } => wma(&candles.closes, *period),
            IndicatorSpec::Vwap => vwap(&candles.closes, &candles.volumes),
            IndicatorSpec::Macd { component, fast, slow, signal_period } => {
                let (m, s, h) = macd(&candles.closes, *fast, *slow, *signal_period);
                match component.as_str() {
                    "signal" => s,
                    "histogram" => h,
                    _ => m,
                }
            },
            IndicatorSpec::Bb { component, period } => {
                let (u, mid, l) = bb(&candles.closes, *period);
                match component.as_str() {
                    "middle" => mid,
                    "lower" => l,
                    _ => u,
                }
            },
            IndicatorSpec::Atr { period } => atr(&candles.highs, &candles.lows, &candles.closes, *period),
            IndicatorSpec::Stoch { period } => stoch(&candles.highs, &candles.lows, &candles.closes, *period),
            IndicatorSpec::Obv => obv(&candles.closes, &candles.volumes),
            IndicatorSpec::Cci { period } => cci(&candles.highs, &candles.lows, &candles.closes, *period),
        };
        out.push((spec.name(), series));
    }
    out
}

/// Apply stationary transformations to raw indicator series.
/// This makes features regime-independent and bounded, which is critical for RL.
pub fn stationary_transform(
    candles: &Candles,
    raw_indicators: &[(String, Vec<f64>)],
) -> Vec<(String, Vec<f64>)> {
    use std::collections::HashMap;

    let n = candles.closes.len();
    let mut result = vec![];

    // Group BB components by period so we can compute %B and bandwidth
    let mut bb_groups: HashMap<usize, (Vec<f64>, Vec<f64>, Vec<f64>)> = HashMap::new();

    for (name, values) in raw_indicators {
        if name.starts_with("ema_") {
            // EMA distance from price: (Close - EMA) / EMA
            // This is naturally stationary (percentage) and zero-centered when price = EMA
            let dist: Vec<f64> = candles
                .closes
                .iter()
                .zip(values.iter())
                .map(|(c, ema)| (c - ema) / ema.max(1e-8))
                .collect();
            result.push((format!("{}_dist", name), dist));
        } else if name.starts_with("rsi_") {
            // RSI centered to [-1, 1]: (RSI - 50) / 50
            // RSI is already bounded [0, 100]; this makes it zero-centered for the MLP
            let centered: Vec<f64> = values.iter().map(|v| (v - 50.0) / 50.0).collect();
            result.push((format!("{}_centered", name), centered));
        } else if name.starts_with("bb_") {
            // Parse bb_{period}_{component} (e.g., bb_20_upper)
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                if let Ok(period) = parts[1].parse::<usize>() {
                    let component = parts[2];
                    let entry = bb_groups.entry(period).or_insert_with(|| {
                        (vec![f64::NAN; n], vec![f64::NAN; n], vec![f64::NAN; n])
                    });
                    match component {
                        "upper" => entry.0 = values.clone(),
                        "middle" => entry.1 = values.clone(),
                        "lower" => entry.2 = values.clone(),
                        _ => {}
                    }
                }
            }
        } else if name == "obv" {
            // OBV delta normalized by rolling standard deviation
            // Raw OBV is cumulative and non-stationary; delta captures flow direction
            let window = 20usize;
            let mut deltas = vec![0.0; n];
            for i in 1..n {
                deltas[i] = values[i] - values[i - 1];
            }
            let mut normalized = vec![f64::NAN; n];
            for i in window..n {
                let win = &deltas[i - window + 1..=i];
                let mean = win.iter().sum::<f64>() / window as f64;
                let std = (win.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / window as f64)
                    .sqrt()
                    .max(1e-8);
                normalized[i] = (deltas[i] - mean) / std;
            }
            result.push(("obv_delta_norm".into(), normalized));
        } else {
            // Pass through any other indicators unchanged (backward compatibility)
            result.push((name.clone(), values.clone()));
        }
    }

    // Process grouped BB components into %B and bandwidth
    for (period, (upper, middle, lower)) in bb_groups {
        // Bandwidth: (Upper - Lower) / Middle
        // Detects volatility squeezes; stationary because it's a ratio
        let bandwidth: Vec<f64> = upper
            .iter()
            .zip(middle.iter())
            .zip(lower.iter())
            .map(|((u, m), l)| (u - l) / m.max(1e-8))
            .collect();
        result.push((format!("bb_{}_bandwidth", period), bandwidth));

        // %B: (Close - Lower) / (Upper - Lower)
        // Where is price within the bands? 0.0 = lower, 1.0 = upper, 0.5 = middle
        let percent_b: Vec<f64> = candles
            .closes
            .iter()
            .zip(upper.iter())
            .zip(lower.iter())
            .map(|((c, u), l)| {
                let range = u - l;
                if range < 1e-8 {
                    0.5
                } else {
                    (c - l) / range
                }
            })
            .collect();
        result.push((format!("bb_{}_percent_b", period), percent_b));
    }

    result
}

pub fn build_state_matrix_with_indices(
    candles: &Candles,
    indicator_series: &[(String, Vec<f64>)],
    lookback: usize,
) -> (Array2<f64>, Vec<String>, Vec<usize>) {
    let n = candles.closes.len();
    let ind_count = indicator_series.len();
    let ohlcv_count = 5 * lookback;
    let state_dim = ind_count + ohlcv_count;

    let mut feature_names: Vec<String> = indicator_series.iter().map(|(n, _)| n.clone()).collect();
    for lag in 1..=lookback {
        for col in ["open", "high", "low", "close", "volume"] {
            feature_names.push(format!("{}_ret_t-{}", col, lag));
        }
    }

    let mut rows: Vec<Array1<f64>> = vec![];
    let mut candle_indices: Vec<usize> = vec![];

    for i in lookback..n {
        let ind_vals: Vec<f64> = indicator_series.iter().map(|(_, v)| v[i]).collect();
        if ind_vals.iter().any(|v| v.is_nan()) { continue; }

        let mut state = Array1::zeros(state_dim);
        for (j, &v) in ind_vals.iter().enumerate() {
            state[j] = v;
        }
        let mut off = ind_count;
        let current_close = candles.closes[i].max(1e-8);
        let current_vol = candles.volumes[i].max(1e-8);
        for lag in 1..=lookback {
            let t = i - lag;
            state[off] = (candles.opens[t] - current_close) / current_close;   off += 1;
            state[off] = (candles.highs[t] - current_close) / current_close;   off += 1;
            state[off] = (candles.lows[t] - current_close) / current_close;    off += 1;
            state[off] = (candles.closes[t] - current_close) / current_close;  off += 1;
            state[off] = (candles.volumes[t] - current_vol) / current_vol;     off += 1;
        }
        rows.push(state);
        candle_indices.push(i);
    }

    if rows.is_empty() {
        return (Array2::zeros((0, state_dim)), feature_names, candle_indices);
    }

    let nrows = rows.len();
    let mut mat = Array2::zeros((nrows, state_dim));
    for (i, row) in rows.into_iter().enumerate() {
        mat.row_mut(i).assign(&row);
    }
    (mat, feature_names, candle_indices)
}

pub fn normalise_with_stats(mat: &mut Array2<f64>) -> (Vec<f64>, Vec<f64>) {
    let ncols = mat.ncols();
    let mut means = vec![0.0f64; ncols];
    let mut stds = vec![1.0f64; ncols];
    for j in 0..ncols {
        let col: Vec<f64> = mat.column(j).to_vec();
        let mean = col.iter().sum::<f64>() / col.len() as f64;
        let std = (col.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / col.len() as f64).sqrt();
        let std = if std < 1e-8 { 1.0 } else { std };
        means[j] = mean;
        stds[j] = std;
        for v in mat.column_mut(j).iter_mut() {
            *v = (*v - mean) / std;
        }
    }
    (means, stds)
}

pub fn apply_normalisation_stats(mat: &mut Array2<f64>, means: &[f64], stds: &[f64]) -> Result<(), String> {
    if mat.ncols() != means.len() || mat.ncols() != stds.len() {
        return Err(format!(
            "normalisation stat dimension mismatch: matrix has {}, means {}, stds {}",
            mat.ncols(),
            means.len(),
            stds.len(),
        ));
    }

    for j in 0..mat.ncols() {
        let std = if stds[j].abs() < 1e-8 { 1.0 } else { stds[j] };
        for v in mat.column_mut(j).iter_mut() {
            *v = (*v - means[j]) / std;
        }
    }
    Ok(())
}
