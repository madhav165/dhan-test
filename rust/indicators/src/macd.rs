use crate::ema::ema;

/// MACD indicator.
/// Returns (macd_line, signal_line, histogram), each of length n.
/// Values are NaN until enough data is available.
pub fn macd(prices: &[f64], fast: usize, slow: usize, signal: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = prices.len();
    let nan = vec![f64::NAN; n];
    if fast >= slow || slow > n {
        return (nan.clone(), nan.clone(), nan);
    }

    let fast_ema = ema(prices, fast);
    let slow_ema = ema(prices, slow);

    let mut macd_line = vec![f64::NAN; n];
    for i in 0..n {
        if !fast_ema[i].is_nan() && !slow_ema[i].is_nan() {
            macd_line[i] = fast_ema[i] - slow_ema[i];
        }
    }

    // signal = EMA of macd_line, seeded from first non-NaN
    let first_valid = macd_line.iter().position(|v| !v.is_nan()).unwrap_or(n);
    let mut signal_line = vec![f64::NAN; n];
    let mut histogram = vec![f64::NAN; n];

    if first_valid + signal <= n {
        let k = 2.0 / (signal as f64 + 1.0);
        let seed_end = first_valid + signal;
        let seed: f64 = macd_line[first_valid..seed_end].iter().sum::<f64>() / signal as f64;
        signal_line[seed_end - 1] = seed;
        for i in seed_end..n {
            signal_line[i] = macd_line[i] * k + signal_line[i - 1] * (1.0 - k);
        }
        for i in 0..n {
            if !macd_line[i].is_nan() && !signal_line[i].is_nan() {
                histogram[i] = macd_line[i] - signal_line[i];
            }
        }
    }

    (macd_line, signal_line, histogram)
}
