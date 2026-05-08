import { db } from '$lib/server/db'
import { error } from '@sveltejs/kit'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select r.id, r.interval, r.from_date, r.to_date, r.run_at,
		        r.num_trades, r.total_pnl, r.win_rate, r.max_drawdown, r.result_key,
		        s.id as strategy_id, s.name as strategy_name,
		        j.status as job_status, j.error as job_error,
		        array_agg(
		          json_build_object(
		            'trading_symbol', i.trading_symbol,
		            'security_id', ri.security_id,
		            'exchange_segment', ri.exchange_segment
		          ) order by i.trading_symbol
		        ) as instruments
		 from backtest_runs r
		 join strategies s on s.id = r.strategy_id
		 left join lateral (
		   select status, error from run_jobs where run_id = r.id order by created_at desc limit 1
		 ) j on true
		 left join backtest_run_instruments ri on ri.run_id = r.id
		 left join instruments i on i.security_id = ri.security_id and i.exchange_segment = ri.exchange_segment
		 where r.id = $1 and s.user_id = $2
		 group by r.id, s.id, j.status, j.error`,
		[params.run_id, locals.user!.id]
	)

	if (result.rows.length === 0) error(404, 'Run not found')

	return { run: result.rows[0] }
}
