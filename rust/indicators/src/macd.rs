use ta::indicators::MovingAverageConvergenceDivergence;
use ta::Next;

pub fn macd(prices: &[f64], fast: usize, slow: usize, signal: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = prices.len();
    let nan = vec![f64::NAN; n];
    let Ok(mut ind) = MovingAverageConvergenceDivergence::new(fast, slow, signal) else {
        return (nan.clone(), nan.clone(), nan);
    };
    let warmup = slow + signal - 1;
    let mut macd_line = vec![f64::NAN; n];
    let mut signal_line = vec![f64::NAN; n];
    let mut histogram = vec![f64::NAN; n];
    for (i, &p) in prices.iter().enumerate() {
        let out = ind.next(p);
        if i + 1 >= warmup {
            macd_line[i] = out.macd;
            signal_line[i] = out.signal;
            histogram[i] = out.histogram;
        }
    }
    (macd_line, signal_line, histogram)
}
