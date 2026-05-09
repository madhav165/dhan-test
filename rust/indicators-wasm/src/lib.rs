use indicators::sma::sma;
use indicators::ema::ema;
use indicators::wma::wma;
use indicators::rsi::rsi;
use indicators::macd::macd;
use indicators::bb::bb;
use indicators::vwap::vwap;
use indicators::atr::atr;
use indicators::stoch::stoch;
use indicators::obv::obv;
use indicators::cci::cci;
use std::alloc::{alloc, dealloc, Layout};

#[unsafe(no_mangle)]
pub extern "C" fn alloc_f64(len: u32) -> *mut f64 {
    let layout = Layout::array::<f64>(len as usize).unwrap();
    unsafe { alloc(layout) as *mut f64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc_f64(ptr: *mut f64, len: u32) {
    let layout = Layout::array::<f64>(len as usize).unwrap();
    unsafe { dealloc(ptr as *mut u8, layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn sma_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    to_output(sma(prices, period as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn ema_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    to_output(ema(prices, period as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn wma_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    to_output(wma(prices, period as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn rsi_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    to_output(rsi(prices, period as usize))
}

/// Returns interleaved [macd, signal, histogram] × n (stride 3, total len = 3*n).
#[unsafe(no_mangle)]
pub extern "C" fn macd_run(ptr: *const f64, len: u32, fast: u32, slow: u32, signal: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let (m, s, h) = macd(prices, fast as usize, slow as usize, signal as usize);
    let n = len as usize;
    let mut out = Vec::with_capacity(n * 3);
    for i in 0..n {
        out.push(m[i]);
        out.push(s[i]);
        out.push(h[i]);
    }
    to_output(out)
}

/// Returns interleaved [upper, middle, lower] × n (stride 3, total len = 3*n).
#[unsafe(no_mangle)]
pub extern "C" fn bb_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let (u, m, l) = bb(prices, period as usize);
    let n = len as usize;
    let mut out = Vec::with_capacity(n * 3);
    for i in 0..n {
        out.push(u[i]);
        out.push(m[i]);
        out.push(l[i]);
    }
    to_output(out)
}

/// price_ptr = typical price array, vol_ptr = volume array, both length len.
#[unsafe(no_mangle)]
pub extern "C" fn vwap_run(price_ptr: *const f64, vol_ptr: *const f64, len: u32) -> *mut f64 {
    let typical = unsafe { std::slice::from_raw_parts(price_ptr, len as usize) };
    let volume = unsafe { std::slice::from_raw_parts(vol_ptr, len as usize) };
    to_output(vwap(typical, volume))
}

/// high_ptr, low_ptr, close_ptr — all length len.
#[unsafe(no_mangle)]
pub extern "C" fn atr_run(high_ptr: *const f64, low_ptr: *const f64, close_ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let highs = unsafe { std::slice::from_raw_parts(high_ptr, len as usize) };
    let lows = unsafe { std::slice::from_raw_parts(low_ptr, len as usize) };
    let closes = unsafe { std::slice::from_raw_parts(close_ptr, len as usize) };
    to_output(atr(highs, lows, closes, period as usize))
}

/// high_ptr, low_ptr, close_ptr — all length len.
#[unsafe(no_mangle)]
pub extern "C" fn stoch_run(high_ptr: *const f64, low_ptr: *const f64, close_ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let highs = unsafe { std::slice::from_raw_parts(high_ptr, len as usize) };
    let lows = unsafe { std::slice::from_raw_parts(low_ptr, len as usize) };
    let closes = unsafe { std::slice::from_raw_parts(close_ptr, len as usize) };
    to_output(stoch(highs, lows, closes, period as usize))
}

/// close_ptr, vol_ptr — both length len.
#[unsafe(no_mangle)]
pub extern "C" fn obv_run(close_ptr: *const f64, vol_ptr: *const f64, len: u32) -> *mut f64 {
    let closes = unsafe { std::slice::from_raw_parts(close_ptr, len as usize) };
    let volumes = unsafe { std::slice::from_raw_parts(vol_ptr, len as usize) };
    to_output(obv(closes, volumes))
}

/// high_ptr, low_ptr, close_ptr — all length len.
#[unsafe(no_mangle)]
pub extern "C" fn cci_run(high_ptr: *const f64, low_ptr: *const f64, close_ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let highs = unsafe { std::slice::from_raw_parts(high_ptr, len as usize) };
    let lows = unsafe { std::slice::from_raw_parts(low_ptr, len as usize) };
    let closes = unsafe { std::slice::from_raw_parts(close_ptr, len as usize) };
    to_output(cci(highs, lows, closes, period as usize))
}

fn to_output(mut v: Vec<f64>) -> *mut f64 {
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    ptr
}
