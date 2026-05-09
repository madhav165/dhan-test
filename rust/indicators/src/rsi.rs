use ta::indicators::RelativeStrengthIndex;
use ta::Next;

pub fn rsi(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    let Ok(mut ind) = RelativeStrengthIndex::new(period) else { return out };
    for (i, &p) in prices.iter().enumerate() {
        let v = ind.next(p);
        if i + 1 >= period {
            out[i] = v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_all_gains_near_100() {
        let prices: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let result = rsi(&prices, 14);
        assert!(result[13] > 95.0, "expected RSI near 100, got {}", result[13]);
    }

    #[test]
    fn rsi_all_losses_near_0() {
        let prices: Vec<f64> = (0..20).map(|i| 20.0 - i as f64).collect();
        let result = rsi(&prices, 14);
        assert!(result[13] < 5.0, "expected RSI near 0, got {}", result[13]);
    }

    #[test]
    fn rsi_nan_before_warmup() {
        let prices: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let result = rsi(&prices, 14);
        assert!(result[12].is_nan());
        assert!(!result[13].is_nan());
    }
}
