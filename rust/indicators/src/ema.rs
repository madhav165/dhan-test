use ta::indicators::ExponentialMovingAverage;
use ta::Next;

pub fn ema(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    let Ok(mut ind) = ExponentialMovingAverage::new(period) else { return out };
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
    fn ema_flat_prices() {
        let prices = vec![10.0f64; 10];
        let result = ema(&prices, 3);
        for &v in &result[2..] {
            assert!((v - 10.0).abs() < 1e-9);
        }
    }
}
