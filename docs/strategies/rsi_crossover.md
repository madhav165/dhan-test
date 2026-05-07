# RSI Crossover

## Logic

Uses the 14-period Relative Strength Index (RSI) on closing prices.

- **Buy** when RSI crosses below 30 (oversold)
- **Sell** when RSI crosses above 70 (overbought)
- **Hold** otherwise

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| period | 14 | RSI lookback window |
| oversold | 30 | Buy threshold |
| overbought | 70 | Sell threshold |

## WASM interface

Input: OHLCV candles as Arrow IPC stream
Output: signal per candle — `buy`, `sell`, or `hold`

## Notes

- Requires at least `period + 1` candles before emitting a signal
- Works on any interval (daily, intraday)
- Mean reversion strategy — performs better in ranging markets, not trending ones
