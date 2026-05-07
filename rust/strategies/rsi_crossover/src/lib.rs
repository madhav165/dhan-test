// RSI Crossover strategy
// Buy when RSI(14) crosses below 30, sell when it crosses above 70, hold otherwise.
//
// WASM interface (single-threaded, no allocator needed):
//   alloc(len: u32) -> *mut f64   — host calls to get a writable buffer for close prices
//   run(len: u32) -> *mut u8      — host calls after writing prices; returns signal buffer
//                                   signals: 0=hold, 1=buy, 2=sell (one byte per candle)

use std::cell::UnsafeCell;

const PERIOD: usize = 14;
const OVERSOLD: f64 = 30.0;
const OVERBOUGHT: f64 = 70.0;

struct State {
    prices: Vec<f64>,
    signals: Vec<u8>,
}

struct WasmState(UnsafeCell<State>);
unsafe impl Sync for WasmState {}

static STATE: WasmState = WasmState(UnsafeCell::new(State {
    prices: Vec::new(),
    signals: Vec::new(),
}));

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut f64 {
    let state = unsafe { &mut *STATE.0.get() };
    state.prices = vec![0.0; len as usize];
    state.prices.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn run(len: u32) -> *mut u8 {
    let state = unsafe { &mut *STATE.0.get() };
    let n = len as usize;
    state.signals = vec![0u8; n];

    if n <= PERIOD {
        return state.signals.as_mut_ptr();
    }

    let rsi = compute_rsi(&state.prices[..n], PERIOD);

    for i in 1..n {
        let prev = rsi[i - 1];
        let curr = rsi[i];
        if prev >= OVERSOLD && curr < OVERSOLD {
            state.signals[i] = 1; // buy
        } else if prev <= OVERBOUGHT && curr > OVERBOUGHT {
            state.signals[i] = 2; // sell
        }
    }

    state.signals.as_mut_ptr()
}

fn compute_rsi(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut rsi = vec![50.0f64; n];

    if n <= period {
        return rsi;
    }

    let mut avg_gain = 0.0f64;
    let mut avg_loss = 0.0f64;
    for i in 1..=period {
        let diff = prices[i] - prices[i - 1];
        if diff > 0.0 {
            avg_gain += diff;
        } else {
            avg_loss += -diff;
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    rsi[period] = rs_to_rsi(avg_gain, avg_loss);

    for i in (period + 1)..n {
        let diff = prices[i] - prices[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;
        rsi[i] = rs_to_rsi(avg_gain, avg_loss);
    }

    rsi
}

fn rs_to_rsi(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        return 100.0;
    }
    100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_no_loss_returns_100() {
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let rsi = compute_rsi(&prices, 14);
        assert!((rsi[14] - 100.0).abs() < 0.001);
    }

    #[test]
    fn signals_length_matches_input() {
        let prices = vec![
            44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.15,
            43.61, 44.33, 44.83, 45.10, 45.15, 43.61, 44.33, 44.83,
            45.85, 46.08, 45.89, 46.03,
        ];
        let n = prices.len() as u32;
        let ptr = alloc(n);
        unsafe {
            for (i, &p) in prices.iter().enumerate() {
                *ptr.add(i) = p;
            }
        }
        run(n);
        let state = unsafe { &*STATE.0.get() };
        assert_eq!(state.signals.len(), prices.len());
    }
}
