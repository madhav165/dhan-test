import { db } from '$lib/server/db'
import type { RequestHandler } from './$types'

export const POST: RequestHandler = async ({ locals }) => {
	if (!locals.user) return new Response('Unauthorized', { status: 401 })

	await db.query(
		`update broker_connections set is_active = false, encrypted_token = null
		 where user_id = $1 and broker = 'dhan'`,
		[locals.user.id]
	)

	return new Response(null, { status: 204 })
}
