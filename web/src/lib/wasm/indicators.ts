type IndicatorFn = (ptr: number, len: number, period: number) => number

interface IndicatorsWasm {
	alloc_f64: (len: number) => number
	dealloc_f64: (ptr: number, len: number) => void
	sma_run: IndicatorFn
	ema_run: IndicatorFn
	rsi_run: IndicatorFn
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
	// output buffer is leaked intentionally (forget pattern in Rust) — GC handles WASM memory

	return out
}
