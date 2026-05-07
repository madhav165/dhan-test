import { db } from '$lib/server/db'
import { redirect, fail } from '@sveltejs/kit'
import type { Actions } from './$types'

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()

		if (!name) return fail(400, { error: 'Name is required' })

		const result = await db.query(
			`insert into strategies (user_id, name) values ($1, $2) returning id`,
			[locals.user!.id, name]
		)

		redirect(302, `/strategies/${result.rows[0].id}`)
	},
}
