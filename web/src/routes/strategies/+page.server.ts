import { db } from '$lib/server/db'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals }) => {
	const result = await db.query(
		`select id, name, wasm_key, created_at
		 from strategies
		 where user_id = $1
		 order by created_at desc`,
		[locals.user!.id]
	)
	return { strategies: result.rows }
}
