import { db } from '$lib/server/db'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals }) => {
	const result = await db.query(
		`select id, name, wasm_key, strategy_type, created_at
		 from strategies
		 where user_id = $1
		 order by created_at desc`,
		[locals.user!.id]
	)
	return { strategies: result.rows }
}

export const actions: Actions = {
	delete: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string
		await db.query(`delete from strategies where id = $1 and user_id = $2`, [id, locals.user!.id])
	},
}
