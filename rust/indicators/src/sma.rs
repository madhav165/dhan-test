use ta::indicators::SimpleMovingAverage;
use ta::Next;

pub fn sma(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut out = vec![f64::NAN; n];
    let Ok(mut ind) = SimpleMovingAverage::new(period) else { return out };
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
