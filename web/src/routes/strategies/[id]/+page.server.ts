import { db } from '$lib/server/db'
import { error } from '@sveltejs/kit'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const stratResult = await db.query(
		`select id, name, interval, from_date, to_date, status, mode, created_at
		 from strategies
		 where id = $1 and user_id = $2`,
		[params.id, locals.user!.id]
	)

	if (stratResult.rows.length === 0) error(404, 'Strategy not found')

	const instrumentsResult = await db.query(
		`select si.security_id, si.exchange_segment, i.trading_symbol, i.custom_symbol
		 from strategy_instruments si
		 left join instruments i on i.security_id = si.security_id and i.exchange_segment = si.exchange_segment
		 where si.strategy_id = $1`,
		[params.id]
	)

	const runsResult = await db.query(
		`select id, run_at, num_trades, total_pnl, win_rate, max_drawdown
		 from backtest_runs
		 where strategy_id = $1
		 order by run_at desc`,
		[params.id]
	)

	return {
		strategy: stratResult.rows[0],
		instruments: instrumentsResult.rows,
		runs: runsResult.rows,
	}
}
