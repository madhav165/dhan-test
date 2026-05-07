import { db } from '$lib/server/db'
import { error, fail, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select id, name, wasm_key from strategies where id = $1 and user_id = $2`,
		[params.id, locals.user!.id]
	)
	if (result.rows.length === 0) error(404, 'Strategy not found')
	if (!result.rows[0].wasm_key) error(400, 'Strategy not compiled yet')

	return { strategy: result.rows[0] }
}

export const actions: Actions = {
	default: async ({ request, locals, params }) => {
		const form = await request.formData()
		const interval = form.get('interval')?.toString()
		const from_date = form.get('from_date')?.toString()
		const to_date = form.get('to_date')?.toString()
		const instruments = form.getAll('instruments').map((v) => JSON.parse(v.toString()))

		if (!interval || !from_date || !to_date) return fail(400, { error: 'All fields are required' })
		if (instruments.length === 0) return fail(400, { error: 'Select at least one instrument' })

		const runResult = await db.query(
			`insert into backtest_runs (strategy_id, interval, from_date, to_date)
			 values ($1, $2, $3, $4) returning id`,
			[params.id, interval, from_date, to_date]
		)
		const runId = runResult.rows[0].id

		for (const inst of instruments) {
			await db.query(
				`insert into backtest_run_instruments (run_id, security_id, exchange_segment)
				 values ($1, $2, $3)`,
				[runId, inst.security_id, inst.exchange_segment]
			)
		}

		await db.query(`insert into run_jobs (run_id) values ($1)`, [runId])

		redirect(302, `/strategies/${params.id}`)
	},
}
