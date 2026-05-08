import type { Condition, Group, Indicator, Operand, RuleSet } from './types/rules'

function indicatorKey(ind: Indicator): string {
	if (ind.name === 'macd') return `macd_${ind.fast}_${ind.slow}_${ind.signal_period}`
	if (ind.name === 'bb') return `bb_${ind.period}`
	if (ind.name === 'vwap') return 'vwap'
	return `${ind.name}_${ind.period}`
}

function operandExpr(op: Operand, suffix: 'curr' | 'prev'): string {
	const idx = suffix === 'prev' ? 'i - 1' : 'i'
	if (op.kind === 'number') return `${op.value}.0`
	if (op.kind === 'raw') {
		if (op.input.kind === 'price') return `prices[${idx}]`
		return `volumes[${idx}]`
	}
	const ind = op.indicator
	const key = indicatorKey(ind)
	if (ind.name === 'macd') return `${key}_${ind.component}[${idx}]`
	if (ind.name === 'bb') return `${key}_${ind.component}[${idx}]`
	return `${key}[${idx}]`
}

function collectAllIndicators(rs: RuleSet): Map<string, Indicator> {
	const map = new Map<string, Indicator>()
	function walk(g: Group) {
		for (const item of g.items) {
			if (item.type === 'condition') {
				for (const op of [item.left, item.right]) {
					if (op.kind === 'indicator') map.set(indicatorKey(op.indicator), op.indicator)
				}
			} else walk(item)
		}
	}
	walk(rs.buy)
	walk(rs.sell)
	return map
}

function emitDeclarations(indicators: Map<string, Indicator>): string {
	const lines: string[] = []
	for (const [key, ind] of indicators) {
		if (ind.name === 'vwap') {
			lines.push(`    let ${key} = vwap(prices, volumes);`)
		} else if (ind.name === 'rsi' || ind.name === 'sma' || ind.name === 'ema') {
			lines.push(`    let ${key} = ${ind.name}(prices, ${ind.period});`)
		} else if (ind.name === 'macd') {
			lines.push(`    let (${key}_macd, ${key}_signal, ${key}_histogram) = macd(prices, ${ind.fast}, ${ind.slow}, ${ind.signal_period});`)
		} else if (ind.name === 'bb') {
			lines.push(`    let (${key}_upper, ${key}_middle, ${key}_lower) = bb(prices, ${ind.period});`)
		}
	}
	return lines.join('\n')
}

function emitConditionExpr(cond: Condition): string {
	const op = cond.operator
	if (op === 'crosses_above') {
		const lp = operandExpr(cond.left, 'prev'), lc = operandExpr(cond.left, 'curr')
		const rp = operandExpr(cond.right, 'prev'), rc = operandExpr(cond.right, 'curr')
		return `(${lp} < ${rp} && ${lc} >= ${rc})`
	}
	if (op === 'crosses_below') {
		const lp = operandExpr(cond.left, 'prev'), lc = operandExpr(cond.left, 'curr')
		const rp = operandExpr(cond.right, 'prev'), rc = operandExpr(cond.right, 'curr')
		return `(${lp} > ${rp} && ${lc} <= ${rc})`
	}
	const l = operandExpr(cond.left, 'curr')
	const r = operandExpr(cond.right, 'curr')
	return `(${l} ${op} ${r})`
}

function emitGroupExpr(group: Group): string | null {
	if (group.items.length === 0) return null
	const parts: string[] = []
	for (const item of group.items) {
		if (item.type === 'condition') {
			parts.push(emitConditionExpr(item))
		} else {
			const sub = emitGroupExpr(item)
			if (sub) parts.push(`(${sub})`)
		}
	}
	if (parts.length === 0) return null
	return parts.join(group.logic === 'AND' ? ' && ' : ' || ')
}

export function ruleToRust(rs: RuleSet): string {
	const indicators = collectAllIndicators(rs)
	const decls = emitDeclarations(indicators)
	const buyExpr = emitGroupExpr(rs.buy)
	const sellExpr = emitGroupExpr(rs.sell)
	const lines: string[] = []
	if (decls) lines.push(decls)
	if (buyExpr) lines.push(`    if ${buyExpr} { return 1; }`)
	if (sellExpr) lines.push(`    if ${sellExpr} { return 2; }`)
	lines.push(`    0`)
	return lines.join('\n')
}
