/// VWAP — Volume Weighted Average Price.
/// typical_price = (high + low + close) / 3
/// Cumulative from the start of the slice (treat as single session).
/// Returns a vec of length n.
pub fn vwap(typical: &[f64], volume: &[f64]) -> Vec<f64> {
    let n = typical.len().min(volume.len());
    let mut out = vec![f64::NAN; n];
    let mut cum_tp_vol = 0.0_f64;
    let mut cum_vol = 0.0_f64;
    for i in 0..n {
        cum_tp_vol += typical[i] * volume[i];
        cum_vol += volume[i];
        out[i] = if cum_vol > 0.0 { cum_tp_vol / cum_vol } else { f64::NAN };
    }
    out
}
