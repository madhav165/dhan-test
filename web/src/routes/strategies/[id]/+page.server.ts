import { db } from '$lib/server/db'
import { error } from '@sveltejs/kit'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const stratResult = await db.query(
		`select s.id, s.name, s.source_key, s.wasm_key, s.created_at,
		        j.status as build_status, j.error as build_error
		 from strategies s
		 left join build_jobs j on j.strategy_id = s.id
		 where s.id = $1 and s.user_id = $2
		 order by j.created_at desc
		 limit 1`,
		[params.id, locals.user!.id]
	)

	if (stratResult.rows.length === 0) error(404, 'Strategy not found')

	const runsResult = await db.query(
		`select r.id, r.interval, r.from_date, r.to_date, r.run_at,
		        r.num_trades, r.total_pnl, r.win_rate, r.max_drawdown,
		        array_agg(i.trading_symbol order by i.trading_symbol) as symbols
		 from backtest_runs r
		 left join backtest_run_instruments ri on ri.run_id = r.id
		 left join instruments i on i.security_id = ri.security_id and i.exchange_segment = ri.exchange_segment
		 where r.strategy_id = $1
		 group by r.id
		 order by r.run_at desc`,
		[params.id]
	)

	const policiesResult = await db.query(
		`select p.id, p.mode, p.status, p.created_at,
		        array_agg(i.trading_symbol order by i.trading_symbol) as symbols
		 from policies p
		 left join policy_instruments pi on pi.policy_id = p.id
		 left join instruments i on i.security_id = pi.security_id and i.exchange_segment = pi.exchange_segment
		 where p.strategy_id = $1
		 group by p.id
		 order by p.created_at desc`,
		[params.id]
	)

	return {
		strategy: stratResult.rows[0],
		runs: runsResult.rows,
		policies: policiesResult.rows,
	}
}
