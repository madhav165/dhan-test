use ta::indicators::BollingerBands;
use ta::Next;

pub fn bb(prices: &[f64], period: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = prices.len();
    let nan = vec![f64::NAN; n];
    let Ok(mut ind) = BollingerBands::new(period, 2.0) else {
        return (nan.clone(), nan.clone(), nan);
    };
    let mut upper = vec![f64::NAN; n];
    let mut middle = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];
    for (i, &p) in prices.iter().enumerate() {
        let out = ind.next(p);
        if i + 1 >= period {
            upper[i] = out.upper;
            middle[i] = out.average;
            lower[i] = out.lower;
        }
    }
    (upper, middle, lower)
}
