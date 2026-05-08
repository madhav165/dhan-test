use ndarray::{Array1, Array2};

pub struct Candles {
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub closes: Vec<f64>,
    pub volumes: Vec<f64>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "name", rename_all = "lowercase")]
pub enum IndicatorSpec {
    Rsi { period: usize },
    Sma { period: usize },
    Ema { period: usize },
    Vwap,
    Macd { component: String, fast: usize, slow: usize, signal_period: usize },
    Bb { component: String, period: usize },
}

impl IndicatorSpec {
    pub fn name(&self) -> String {
        match self {
            Self::Rsi { period } => format!("rsi_{}", period),
            Self::Sma { period } => format!("sma_{}", period),
            Self::Ema { period } => format!("ema_{}", period),
            Self::Vwap => "vwap".into(),
            Self::Macd { component, fast, slow, signal_period } =>
                format!("macd_{}_{}_{}_{}",fast, slow, signal_period, component),
            Self::Bb { component, period } => format!("bb_{}_{}", period, component),
        }
    }
}

pub fn compute_indicators(candles: &Candles, specs: &[IndicatorSpec]) -> Vec<(String, Vec<f64>)> {
    use indicators::{rsi::rsi, sma::sma, ema::ema, vwap::vwap, macd::macd, bb::bb};
    let mut out = vec![];
    for spec in specs {
        let series: Vec<f64> = match spec {
            IndicatorSpec::Rsi { period } => rsi(&candles.closes, *period),
            IndicatorSpec::Sma { period } => sma(&candles.closes, *period),
            IndicatorSpec::Ema { period } => ema(&candles.closes, *period),
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
        };
        out.push((spec.name(), series));
    }
    out
}

pub fn build_state_matrix(
    candles: &Candles,
    indicator_series: &[(String, Vec<f64>)],
    lookback: usize,
) -> (Array2<f64>, Vec<String>) {
    let n = candles.closes.len();
    let ind_count = indicator_series.len();
    let ohlcv_count = 5 * lookback;
    let state_dim = ind_count + ohlcv_count;

    let mut feature_names: Vec<String> = indicator_series.iter().map(|(n, _)| n.clone()).collect();
    for lag in 1..=lookback {
        for col in ["open", "high", "low", "close", "volume"] {
            feature_names.push(format!("{}_t-{}", col, lag));
        }
    }

    let mut rows: Vec<Array1<f64>> = vec![];

    for i in lookback..n {
        let ind_vals: Vec<f64> = indicator_series.iter().map(|(_, v)| v[i]).collect();
        if ind_vals.iter().any(|v| v.is_nan()) { continue; }

        let mut state = Array1::zeros(state_dim);
        for (j, &v) in ind_vals.iter().enumerate() {
            state[j] = v;
        }
        let mut off = ind_count;
        for lag in 1..=lookback {
            let t = i - lag;
            state[off] = candles.opens[t];   off += 1;
            state[off] = candles.highs[t];   off += 1;
            state[off] = candles.lows[t];    off += 1;
            state[off] = candles.closes[t];  off += 1;
            state[off] = candles.volumes[t]; off += 1;
        }
        rows.push(state);
    }

    if rows.is_empty() {
        return (Array2::zeros((0, state_dim)), feature_names);
    }

    let nrows = rows.len();
    let mut mat = Array2::zeros((nrows, state_dim));
    for (i, row) in rows.into_iter().enumerate() {
        mat.row_mut(i).assign(&row);
    }
    (mat, feature_names)
}

pub fn normalise(mat: &mut Array2<f64>) {
    let ncols = mat.ncols();
    for j in 0..ncols {
        let col: Vec<f64> = mat.column(j).to_vec();
        let mean = col.iter().sum::<f64>() / col.len() as f64;
        let std = (col.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / col.len() as f64).sqrt();
        let std = if std < 1e-8 { 1.0 } else { std };
        for v in mat.column_mut(j).iter_mut() {
            *v = (*v - mean) / std;
        }
    }
}
