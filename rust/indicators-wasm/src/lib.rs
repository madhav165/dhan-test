use indicators::sma::sma;
use indicators::ema::ema;
use indicators::rsi::rsi;
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
pub extern "C" fn rsi_run(ptr: *const f64, len: u32, period: u32) -> *mut f64 {
    let prices = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    to_output(rsi(prices, period as usize))
}

fn to_output(mut v: Vec<f64>) -> *mut f64 {
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    ptr
}
