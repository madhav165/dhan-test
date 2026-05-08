import { db } from '$lib/server/db'
import { error, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select p.id, p.mode, p.interval,
		        json_agg(json_build_object(
		          'security_id', pi.security_id,
		          'exchange_segment', pi.exchange_segment,
		          'trading_symbol', i.trading_symbol,
		          'custom_symbol', i.trading_symbol
		        )) filter (where pi.security_id is not null) as instruments
		 from policies p
		 join strategies s on s.id = p.strategy_id
		 left join policy_instruments pi on pi.policy_id = p.id
		 left join instruments i on i.security_id = pi.security_id and i.exchange_segment = pi.exchange_segment
		 where p.id = $1 and s.user_id = $2
		 group by p.id`,
		[params.policy_id, locals.user!.id]
	)
	if (result.rows.length === 0) error(404, 'Policy not found')
	return { policy: result.rows[0] }
}

export const actions: Actions = {
	default: async ({ request, locals, params }) => {
		const form = await request.formData()
		const mode = form.get('mode')?.toString()
		const interval = form.get('interval')?.toString()
		const instruments = form.getAll('instruments').map((v) => JSON.parse(v.toString()))

		if (!mode || !interval) error(400, 'All fields required')
		if (instruments.length === 0) error(400, 'Select at least one instrument')

		await db.query(
			`update policies set mode = $1, interval = $2
			 where id = $3 and strategy_id in (select id from strategies where user_id = $4)`,
			[mode, interval, params.policy_id, locals.user!.id]
		)
		await db.query(`delete from policy_instruments where policy_id = $1`, [params.policy_id])
		for (const inst of instruments) {
			await db.query(
				`insert into policy_instruments (policy_id, security_id, exchange_segment) values ($1, $2, $3)`,
				[params.policy_id, inst.security_id, inst.exchange_segment]
			)
		}

		redirect(302, `/policies/${params.policy_id}`)
	},
}
