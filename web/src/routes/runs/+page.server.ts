import { GO_URL } from '$env/static/private'
import { db } from '$lib/server/db'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, url }) => {
	const strategyId = url.searchParams.get('strategy_id')

	const result = await db.query(
		`select r.id, r.interval, r.from_date, r.to_date, r.run_at,
		        r.num_trades, r.total_pnl, r.win_rate, r.max_drawdown,
		        s.id as strategy_id, s.name as strategy_name,
		        j.status as job_status,
		        array_agg(i.trading_symbol order by i.trading_symbol) as symbols
		 from backtest_runs r
		 join strategies s on s.id = r.strategy_id
		 left join lateral (
		   select status from run_jobs where run_id = r.id order by created_at desc limit 1
		 ) j on true
		 left join backtest_run_instruments ri on ri.run_id = r.id
		 left join instruments i on i.security_id = ri.security_id and i.exchange_segment = ri.exchange_segment
		 where s.user_id = $1 ${strategyId ? 'and s.id = $2' : ''}
		 group by r.id, s.id, j.status
		 order by r.run_at desc`,
		strategyId ? [locals.user!.id, strategyId] : [locals.user!.id]
	)

	const strategiesResult = await db.query(
		`select id, name from strategies where user_id = $1 and wasm_key is not null order by name`,
		[locals.user!.id]
	)

	return {
		runs: result.rows,
		strategies: strategiesResult.rows,
		selectedStrategyId: strategyId,
	}
}

export const actions: Actions = {
	delete: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string
		await fetch(`${GO_URL}/chart/run?run_id=${id}`, {
			method: 'DELETE',
			headers: { 'X-User-ID': locals.user!.id },
		})
		await db.query(
			`delete from backtest_runs where id = $1
			 and strategy_id in (select id from strategies where user_id = $2)`,
			[id, locals.user!.id]
		)
	}
}
