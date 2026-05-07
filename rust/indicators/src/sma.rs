/// Simple Moving Average over a sliding window.
/// Returns a vec of the same length; first `period-1` values are `f64::NAN`.
pub fn sma(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    let mut sum: f64 = prices[..period].iter().sum();
    out[period - 1] = sum / period as f64;
    for i in period..n {
        sum += prices[i] - prices[i - period];
        out[i] = sum / period as f64;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_basic() {
        let prices = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma(&prices, 3);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 2.0).abs() < 1e-9);
        assert!((result[3] - 3.0).abs() < 1e-9);
        assert!((result[4] - 4.0).abs() < 1e-9);
    }
}
