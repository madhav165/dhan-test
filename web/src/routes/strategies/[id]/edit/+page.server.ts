import { db } from '$lib/server/db'
import { error, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, params }) => {
	const result = await db.query(
		`select id, name, rule_json from strategies where id = $1 and user_id = $2`,
		[params.id, locals.user!.id]
	)
	if (result.rows.length === 0) error(404, 'Strategy not found')
	return { strategy: result.rows[0] }
}

export const actions: Actions = {
	default: async ({ request, locals, params }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const code = form.get('code')?.toString().trim()
		const rule_json = form.get('rule_json')?.toString() ?? null

		if (!name || !code) error(400, 'Name and code are required')

		await db.query(
			`update strategies set name = $1, source_key = $2, wasm_key = null, rule_json = $3 where id = $4 and user_id = $5`,
			[name, code, rule_json ? JSON.parse(rule_json) : null, params.id, locals.user!.id]
		)
		await db.query(`insert into build_jobs (strategy_id) values ($1)`, [params.id])

		redirect(302, `/strategies/${params.id}`)
	},
}
