/// Exponential Moving Average.
/// Returns a vec of the same length; first `period-1` values are `f64::NAN`.
pub fn ema(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    let k = 2.0 / (period as f64 + 1.0);
    // seed with SMA of first period
    let seed: f64 = prices[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = seed;
    for i in period..n {
        out[i] = prices[i] * k + out[i - 1] * (1.0 - k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_flat_prices() {
        let prices = vec![10.0f64; 10];
        let result = ema(&prices, 3);
        // flat prices → EMA equals price
        for &v in &result[2..] {
            assert!((v - 10.0).abs() < 1e-9);
        }
    }
}
