use ta::indicators::AverageTrueRange;
use ta::{DataItem, Next};

pub fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let n = highs.len().min(lows.len()).min(closes.len());
    let mut out = vec![f64::NAN; n];
    let Ok(mut ind) = AverageTrueRange::new(period) else { return out };
    for i in 0..n {
        let item = DataItem::builder()
            .high(highs[i]).low(lows[i]).close(closes[i])
            .open(closes[i]).volume(0.0).build().unwrap();
        let v = ind.next(&item);
        if i + 1 >= period {
            out[i] = v;
        }
    }
    out
}
