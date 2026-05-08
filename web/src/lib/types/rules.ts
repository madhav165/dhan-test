export type RawInput = { kind: 'price' } | { kind: 'volume' }

export type Indicator =
	| { name: 'rsi' | 'sma' | 'ema'; period: number }
	| { name: 'vwap' }
	| { name: 'macd'; component: 'macd' | 'signal' | 'histogram'; fast: number; slow: number; signal_period: number }
	| { name: 'bb'; component: 'upper' | 'middle' | 'lower'; period: number }

export type Operand =
	| { kind: 'raw'; input: RawInput }
	| { kind: 'indicator'; indicator: Indicator }
	| { kind: 'number'; value: number }

export type Operator = '>' | '<' | '>=' | '<=' | '==' | 'crosses_above' | 'crosses_below'

export type Condition = {
	type: 'condition'
	id: string
	left: Operand
	operator: Operator
	right: Operand
}

export type Group = {
	type: 'group'
	id: string
	logic: 'AND' | 'OR'
	items: (Condition | Group)[]
}

export type RuleSet = { buy: Group; sell: Group }

export function emptyGroup(id: string): Group {
	return { type: 'group', id, logic: 'AND', items: [] }
}

export function emptyCondition(id: string): Condition {
	return {
		type: 'condition',
		id,
		left: { kind: 'indicator', indicator: { name: 'rsi', period: 14 } },
		operator: '<',
		right: { kind: 'number', value: 30 },
	}
}
