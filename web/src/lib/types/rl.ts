import type { Indicator } from './rules'

export type RLReward = 'pnl' | 'sharpe' | 'min_drawdown'

export type RLConstraint =
	| { type: 'max_holding_days'; value: number }
	| { type: 'max_trades_per_month'; value: number }

export type RLConfig = {
	reward: RLReward
	constraints: RLConstraint[]
	indicators: Indicator[]
	lookback_candles: number
	allow_short: boolean
	security_id: string
	exchange_segment: string
	trading_symbol: string
	train_from: string  // YYYY-MM-DD, full learning range start
	train_to: string    // YYYY-MM-DD, full learning range end
	test_from?: string  // legacy configs only
	test_to?: string    // legacy configs only
}

export type FeatureImportance = {
	name: string
	importance: number  // 0–1, higher = more influential
}

export type RLSummary = {
	feature_importance: FeatureImportance[]
	approximate_rules: string  // human-readable decision tree text
	training_episodes: number
	best_episode?: number
	final_train_reward: number
	train_pnl?: number
	val_pnl: number       // absolute PnL per unit on the validation split
	test_pnl: number | null  // absolute PnL per unit on the final holdout split
	split?: {
		train_from: string
		train_to: string
		val_from: string
		val_to: string
		test_from: string
		test_to: string
		train_rows: number
		val_rows: number
		test_rows: number
	}
}
