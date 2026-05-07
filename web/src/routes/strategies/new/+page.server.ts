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

		// Store strategy — source_key and wasm_key set after compilation
		const result = await db.query(
			`insert into strategies (user_id, name) values ($1, $2) returning id`,
			[locals.user!.id, name]
		)

		const strategyId = result.rows[0].id

		// TODO: send code to compilation pipeline → MinIO → update source_key/wasm_key
		// For now, store code in DB temporarily until MinIO pipeline is wired
		await db.query(
			`update strategies set source_key = $1 where id = $2`,
			[code, strategyId]
		)

		redirect(302, `/strategies/${strategyId}`)
	},
}
