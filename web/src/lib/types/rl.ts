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
	train_from: string  // YYYY-MM-DD
	train_to: string    // YYYY-MM-DD
	test_from: string   // YYYY-MM-DD — held-out period, never seen during training
	test_to: string     // YYYY-MM-DD
}

export type FeatureImportance = {
	name: string
	importance: number  // 0–1, higher = more influential
}

export type RLSummary = {
	feature_importance: FeatureImportance[]
	approximate_rules: string  // human-readable decision tree text
	training_episodes: number
	final_train_reward: number
	val_pnl: number       // PnL on the 20% validation split (last 20% of train range)
	test_pnl: number | null  // PnL on held-out test range; null if not configured
}
