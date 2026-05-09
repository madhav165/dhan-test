type IndicatorFn = (ptr: number, len: number, period: number) => number

interface IndicatorsWasm {
	alloc_f64: (len: number) => number
	dealloc_f64: (ptr: number, len: number) => void
	sma_run: IndicatorFn
	ema_run: IndicatorFn
	wma_run: IndicatorFn
	rsi_run: IndicatorFn
	macd_run: (ptr: number, len: number, fast: number, slow: number, signal: number) => number
	bb_run: (ptr: number, len: number, period: number) => number
	vwap_run: (price_ptr: number, vol_ptr: number, len: number) => number
	atr_run: (high_ptr: number, low_ptr: number, close_ptr: number, len: number, period: number) => number
	stoch_run: (high_ptr: number, low_ptr: number, close_ptr: number, len: number, period: number) => number
	obv_run: (close_ptr: number, vol_ptr: number, len: number) => number
	cci_run: (high_ptr: number, low_ptr: number, close_ptr: number, len: number, period: number) => number
	memory: WebAssembly.Memory
}

let wasmInstance: IndicatorsWasm | null = null

export async function loadIndicators(): Promise<IndicatorsWasm> {
	if (wasmInstance) return wasmInstance
	const result = await WebAssembly.instantiateStreaming(fetch('/indicators.wasm'))
	wasmInstance = result.instance.exports as unknown as IndicatorsWasm
	return wasmInstance
}

export function runIndicator(
	wasm: IndicatorsWasm,
	fn: IndicatorFn,
	prices: number[],
	period: number
): number[] {
	const ptr = wasm.alloc_f64(prices.length)
	const mem = new Float64Array(wasm.memory.buffer, ptr, prices.length)
	mem.set(prices)
	const outPtr = fn(ptr, prices.length, period)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, prices.length))
	wasm.dealloc_f64(ptr, prices.length)
	return out
}

// Returns [macd_line, signal_line, histogram] — each array of length n
export function runMacd(
	wasm: IndicatorsWasm,
	prices: number[],
	fast = 12, slow = 26, signal = 9
): [number[], number[], number[]] {
	const n = prices.length
	const ptr = wasm.alloc_f64(n)
	new Float64Array(wasm.memory.buffer, ptr, n).set(prices)
	const outPtr = wasm.macd_run(ptr, n, fast, slow, signal)
	const raw = new Float64Array(wasm.memory.buffer, outPtr, n * 3)
	const macdLine = Array.from(raw.filter((_, i) => i % 3 === 0))
	const signalLine = Array.from(raw.filter((_, i) => i % 3 === 1))
	const histogram = Array.from(raw.filter((_, i) => i % 3 === 2))
	wasm.dealloc_f64(ptr, n)
	return [macdLine, signalLine, histogram]
}

// Returns [upper, middle, lower] — each array of length n
export function runBB(
	wasm: IndicatorsWasm,
	prices: number[],
	period = 20
): [number[], number[], number[]] {
	const n = prices.length
	const ptr = wasm.alloc_f64(n)
	new Float64Array(wasm.memory.buffer, ptr, n).set(prices)
	const outPtr = wasm.bb_run(ptr, n, period)
	const raw = new Float64Array(wasm.memory.buffer, outPtr, n * 3)
	const upper = Array.from(raw.filter((_, i) => i % 3 === 0))
	const middle = Array.from(raw.filter((_, i) => i % 3 === 1))
	const lower = Array.from(raw.filter((_, i) => i % 3 === 2))
	wasm.dealloc_f64(ptr, n)
	return [upper, middle, lower]
}

// Returns vwap array of length n. typical = (H+L+C)/3, volume = raw volume
export function runVwap(
	wasm: IndicatorsWasm,
	typical: number[],
	volume: number[]
): number[] {
	const n = typical.length
	const pricePtr = wasm.alloc_f64(n)
	const volPtr = wasm.alloc_f64(n)
	new Float64Array(wasm.memory.buffer, pricePtr, n).set(typical)
	new Float64Array(wasm.memory.buffer, volPtr, n).set(volume)
	const outPtr = wasm.vwap_run(pricePtr, volPtr, n)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, n))
	wasm.dealloc_f64(pricePtr, n)
	wasm.dealloc_f64(volPtr, n)
	return out
}

function allocHLC(wasm: IndicatorsWasm, highs: number[], lows: number[], closes: number[]) {
	const n = highs.length
	const hp = wasm.alloc_f64(n), lp = wasm.alloc_f64(n), cp = wasm.alloc_f64(n)
	new Float64Array(wasm.memory.buffer, hp, n).set(highs)
	new Float64Array(wasm.memory.buffer, lp, n).set(lows)
	new Float64Array(wasm.memory.buffer, cp, n).set(closes)
	return { n, hp, lp, cp }
}

export function runAtr(wasm: IndicatorsWasm, highs: number[], lows: number[], closes: number[], period: number): number[] {
	const { n, hp, lp, cp } = allocHLC(wasm, highs, lows, closes)
	const outPtr = wasm.atr_run(hp, lp, cp, n, period)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, n))
	wasm.dealloc_f64(hp, n); wasm.dealloc_f64(lp, n); wasm.dealloc_f64(cp, n)
	return out
}

export function runStoch(wasm: IndicatorsWasm, highs: number[], lows: number[], closes: number[], period: number): number[] {
	const { n, hp, lp, cp } = allocHLC(wasm, highs, lows, closes)
	const outPtr = wasm.stoch_run(hp, lp, cp, n, period)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, n))
	wasm.dealloc_f64(hp, n); wasm.dealloc_f64(lp, n); wasm.dealloc_f64(cp, n)
	return out
}

export function runObv(wasm: IndicatorsWasm, closes: number[], volumes: number[]): number[] {
	const n = closes.length
	const cp = wasm.alloc_f64(n), vp = wasm.alloc_f64(n)
	new Float64Array(wasm.memory.buffer, cp, n).set(closes)
	new Float64Array(wasm.memory.buffer, vp, n).set(volumes)
	const outPtr = wasm.obv_run(cp, vp, n)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, n))
	wasm.dealloc_f64(cp, n); wasm.dealloc_f64(vp, n)
	return out
}

export function runCci(wasm: IndicatorsWasm, highs: number[], lows: number[], closes: number[], period: number): number[] {
	const { n, hp, lp, cp } = allocHLC(wasm, highs, lows, closes)
	const outPtr = wasm.cci_run(hp, lp, cp, n, period)
	const out = Array.from(new Float64Array(wasm.memory.buffer, outPtr, n))
	wasm.dealloc_f64(hp, n); wasm.dealloc_f64(lp, n); wasm.dealloc_f64(cp, n)
	return out
}
