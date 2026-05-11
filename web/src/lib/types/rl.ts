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
	interval: string
	train_from: string
	train_to: string
	test_from?: string
	test_to?: string
	training_method: 'ppo' | 'reinforce'
	lr: number
	actor_lr: number
	critic_lr: number
	hidden_size: number
	num_layers: number
	activation: 'relu' | 'tanh'
	ppo_epochs: number
	clip_epsilon: number
	value_coef: number
	entropy_coef: number
	gae_lambda: number
	batch_episodes: number
	reward_norm: boolean
	lr_schedule: boolean
	entropy_anneal: boolean
	regularization_type: 'none' | 'l1' | 'l2'
	regularization_lambda: number
	continuous_action: boolean
	action_std: number
	action_penalty: number
	position_deadband: number
	state_mode?: 'baseline' | 'hybrid' | 'lean'
	velocity_lookback?: number
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
	internal_test_pnl?: number
	external_test_pnl?: number | null
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
