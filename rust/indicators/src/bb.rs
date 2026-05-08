/// Bollinger Bands (period, multiplier=2.0).
/// Returns (upper, middle, lower), each of length n.
pub fn bb(prices: &[f64], period: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = prices.len();
    let nan = vec![f64::NAN; n];
    if period == 0 || period > n {
        return (nan.clone(), nan.clone(), nan);
    }

    let mult = 2.0_f64;
    let mut upper = vec![f64::NAN; n];
    let mut middle = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];

    for i in (period - 1)..n {
        let slice = &prices[(i + 1 - period)..=i];
        let mean = slice.iter().sum::<f64>() / period as f64;
        let variance = slice.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / period as f64;
        let std = variance.sqrt();
        middle[i] = mean;
        upper[i] = mean + mult * std;
        lower[i] = mean - mult * std;
    }

    (upper, middle, lower)
}
