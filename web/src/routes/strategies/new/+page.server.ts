import { db } from '$lib/server/db'
import { redirect, fail } from '@sveltejs/kit'
import type { Actions } from './$types'

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const code = form.get('code')?.toString().trim()

		if (!name) return fail(400, { error: 'Name is required' })
		if (!code) return fail(400, { error: 'Signal logic is required' })

		const stratResult = await db.query(
			`insert into strategies (user_id, name) values ($1, $2) returning id`,
			[locals.user!.id, name]
		)
		const strategyId = stratResult.rows[0].id

		// Store raw snippet temporarily — builder will wrap + compile it
		await db.query(
			`update strategies set source_key = $1 where id = $2`,
			[code, strategyId]
		)

		// Queue build job
		await db.query(
			`insert into build_jobs (strategy_id) values ($1)`,
			[strategyId]
		)

		redirect(302, `/strategies/${strategyId}`)
	},
}
