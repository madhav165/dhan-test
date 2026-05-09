use ta::indicators::OnBalanceVolume;
use ta::{DataItem, Next};

pub fn obv(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let n = closes.len().min(volumes.len());
    let mut out = vec![f64::NAN; n];
    let mut ind = OnBalanceVolume::new();
    for i in 0..n {
        let item = DataItem::builder()
            .close(closes[i]).volume(volumes[i])
            .high(closes[i]).low(closes[i]).open(closes[i])
            .build().unwrap();
        out[i] = ind.next(&item);
    }
    out
}
