import { db } from '$lib/server/db'
import { error, fail, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, url }) => {
	const strategyId = url.searchParams.get('strategy_id')

	const result = await db.query(
		`select id, name from strategies where user_id = $1 and wasm_key is not null order by name`,
		[locals.user!.id]
	)

	if (result.rows.length === 0) error(400, 'No compiled strategies available')

	return {
		strategies: result.rows,
		preselectedStrategyId: strategyId,
	}
}

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const strategy_id = form.get('strategy_id')?.toString()
		const mode = form.get('mode')?.toString()
		const instruments = form.getAll('instruments').map((v) => JSON.parse(v.toString()))

		if (!strategy_id || !mode) return fail(400, { error: 'All fields are required' })
		if (instruments.length === 0) return fail(400, { error: 'Select at least one instrument' })
		if (!['alert', 'trade'].includes(mode)) return fail(400, { error: 'Invalid mode' })

		const stratResult = await db.query(
			`select id from strategies where id = $1 and user_id = $2 and wasm_key is not null`,
			[strategy_id, locals.user!.id]
		)
		if (stratResult.rows.length === 0) return fail(400, { error: 'Strategy not found' })

		const policyResult = await db.query(
			`insert into policies (strategy_id, mode) values ($1, $2) returning id`,
			[strategy_id, mode]
		)
		const policyId = policyResult.rows[0].id

		for (const inst of instruments) {
			await db.query(
				`insert into policy_instruments (policy_id, security_id, exchange_segment)
				 values ($1, $2, $3)`,
				[policyId, inst.security_id, inst.exchange_segment]
			)
		}

		redirect(302, `/policies/${policyId}`)
	},
}
