import { db } from '$lib/server/db'
import { error, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const stratResult = await db.query(
		`select s.id, s.name, s.source_key, s.wasm_key, s.created_at,
		        s.strategy_type, s.rl_config, s.rl_summary,
		        j.status as build_status, j.error as build_error
		 from strategies s
		 left join lateral (
		   select status, error from build_jobs
		   where strategy_id = s.id
		   order by created_at desc
		   limit 1
		 ) j on true
		 where s.id = $1 and s.user_id = $2`,
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

	const rlJobResult = await db.query(
		`select status, error from rl_jobs where strategy_id = $1 order by created_at desc limit 1`,
		[params.id]
	)

	const rlMetricsResult = await db.query(
		`select episode, train_reward, val_metric from rl_training_metrics where strategy_id = $1 order by episode`,
		[params.id]
	)

	return {
		strategy: {
			...stratResult.rows[0],
			rl_job: rlJobResult.rows[0] ?? null,
		},
		runs: runsResult.rows,
		policies: policiesResult.rows,
		rl_metrics: rlMetricsResult.rows,
	}
}

export const actions: Actions = {
	delete: async ({ locals, params }) => {
		await db.query(`delete from strategies where id = $1 and user_id = $2`, [params.id, locals.user!.id])
		redirect(303, '/strategies')
	},
}
