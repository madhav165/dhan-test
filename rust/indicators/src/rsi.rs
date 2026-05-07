/// RSI using Wilder's smoothing method.
/// Returns a vec of the same length; first `period` values are `f64::NAN`.
pub fn rsi(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n <= period {
        return out;
    }

    let mut avg_gain = 0.0f64;
    let mut avg_loss = 0.0f64;
    for i in 1..=period {
        let diff = prices[i] - prices[i - 1];
        if diff > 0.0 { avg_gain += diff; } else { avg_loss += -diff; }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;
    out[period] = to_rsi(avg_gain, avg_loss);

    for i in (period + 1)..n {
        let diff = prices[i] - prices[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;
        out[i] = to_rsi(avg_gain, avg_loss);
    }

    out
}

fn to_rsi(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 { return 100.0; }
    100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_all_gains_returns_100() {
        let prices: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let result = rsi(&prices, 14);
        assert!((result[14] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn rsi_all_losses_returns_0() {
        let prices: Vec<f64> = (0..20).map(|i| 20.0 - i as f64).collect();
        let result = rsi(&prices, 14);
        assert!(result[14].abs() < 1e-9);
    }
}
