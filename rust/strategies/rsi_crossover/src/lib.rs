// RSI Crossover strategy
// Buy when RSI(14) crosses below 30, sell when it crosses above 70, hold otherwise.
//
// WASM interface:
//   alloc(len: u32) -> *mut f64   — host writes close prices into returned buffer
//   run(len: u32) -> *mut u8      — returns signal buffer (0=hold, 1=buy, 2=sell)

use std::cell::UnsafeCell;
use indicators::rsi::rsi;

const PERIOD: usize = 14;
const OVERSOLD: f64 = 30.0;
const OVERBOUGHT: f64 = 70.0;

struct State { prices: Vec<f64>, signals: Vec<u8> }
struct WasmState(UnsafeCell<State>);
unsafe impl Sync for WasmState {}

static STATE: WasmState = WasmState(UnsafeCell::new(State {
    prices: Vec::new(),
    signals: Vec::new(),
}));

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.prices = vec![0.0; len as usize];
    s.prices.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn run(len: u32) -> *mut u8 {
    let s = unsafe { &mut *STATE.0.get() };
    let n = len as usize;
    s.signals = vec![0u8; n];

    if n <= PERIOD { return s.signals.as_mut_ptr(); }

    let rsi_vals = rsi(&s.prices[..n], PERIOD);

    for i in 1..n {
        let prev = rsi_vals[i - 1];
        let curr = rsi_vals[i];
        if prev.is_nan() || curr.is_nan() { continue; }
        if prev >= OVERSOLD && curr < OVERSOLD {
            s.signals[i] = 1; // buy
        } else if prev <= OVERBOUGHT && curr > OVERBOUGHT {
            s.signals[i] = 2; // sell
        }
    }

    s.signals.as_mut_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            for (i, &p) in prices.iter().enumerate() { *ptr.add(i) = p; }
        }
        run(n);
        let s = unsafe { &*STATE.0.get() };
        assert_eq!(s.signals.len(), prices.len());
    }
}
