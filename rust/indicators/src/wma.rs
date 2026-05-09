pub fn wma(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n { return out; }
    let denom = (period * (period + 1) / 2) as f64;
    for i in (period - 1)..n {
        let mut sum = 0.0;
        for j in 0..period {
            sum += prices[i - (period - 1 - j)] * (j + 1) as f64;
        }
        out[i] = sum / denom;
    }
    out
}
