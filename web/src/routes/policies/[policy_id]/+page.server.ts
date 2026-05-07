import { db } from '$lib/server/db'
import { error, fail } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select p.id, p.mode, p.status, p.created_at,
		        s.id as strategy_id, s.name as strategy_name,
		        array_agg(
		          json_build_object(
		            'trading_symbol', i.trading_symbol,
		            'exchange_segment', pi.exchange_segment
		          ) order by i.trading_symbol
		        ) as instruments
		 from policies p
		 join strategies s on s.id = p.strategy_id
		 left join policy_instruments pi on pi.policy_id = p.id
		 left join instruments i on i.security_id = pi.security_id and i.exchange_segment = pi.exchange_segment
		 where p.id = $1 and s.user_id = $2
		 group by p.id, s.id`,
		[params.policy_id, locals.user!.id]
	)

	if (result.rows.length === 0) error(404, 'Policy not found')

	return { policy: result.rows[0] }
}

export const actions: Actions = {
	toggle: async ({ locals, params }) => {
		const result = await db.query(
			`select p.status from policies p
			 join strategies s on s.id = p.strategy_id
			 where p.id = $1 and s.user_id = $2`,
			[params.policy_id, locals.user!.id]
		)
		if (result.rows.length === 0) return fail(404, { error: 'Policy not found' })

		const next = result.rows[0].status === 'active' ? 'paused' : 'active'
		await db.query(`update policies set status = $1 where id = $2`, [next, params.policy_id])
		return { status: next }
	},
}
