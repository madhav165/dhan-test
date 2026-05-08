import { db } from '$lib/server/db'
import { redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, url }) => {
	const strategyId = url.searchParams.get('strategy_id')

	const result = await db.query(
		`select p.id, p.mode, p.status, p.interval, p.created_at,
		        s.id as strategy_id, s.name as strategy_name,
		        array_agg(i.trading_symbol order by i.trading_symbol) as symbols
		 from policies p
		 join strategies s on s.id = p.strategy_id
		 left join policy_instruments pi on pi.policy_id = p.id
		 left join instruments i on i.security_id = pi.security_id and i.exchange_segment = pi.exchange_segment
		 where s.user_id = $1 ${strategyId ? 'and s.id = $2' : ''}
		 group by p.id, s.id
		 order by p.created_at desc`,
		strategyId ? [locals.user!.id, strategyId] : [locals.user!.id]
	)

	const strategiesResult = await db.query(
		`select id, name from strategies where user_id = $1 and wasm_key is not null order by name`,
		[locals.user!.id]
	)

	return {
		policies: result.rows,
		strategies: strategiesResult.rows,
		selectedStrategyId: strategyId,
	}
}

export const actions: Actions = {
	toggle: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string
		const current = data.get('status') as string
		const next = current === 'active' ? 'paused' : 'active'
		await db.query(
			`update policies set status = $1
			 where id = $2 and strategy_id in (select id from strategies where user_id = $3)`,
			[next, id, locals.user!.id]
		)
	},

	delete: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string
		await db.query(
			`delete from policies where id = $1
			 and strategy_id in (select id from strategies where user_id = $2)`,
			[id, locals.user!.id]
		)
	},
}
