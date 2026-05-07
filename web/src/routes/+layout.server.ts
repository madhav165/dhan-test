import { redirect } from '@sveltejs/kit'
import { db } from '$lib/server/db'
import type { LayoutServerLoad } from './$types'

export const load: LayoutServerLoad = async ({ locals, url }) => {
	const isAuthRoute = url.pathname.startsWith('/auth') || url.pathname.startsWith('/login')
	if (!locals.user && !isAuthRoute) redirect(302, '/login')

	let brokerConnected = false
	if (locals.user) {
		const result = await db.query(
			`select is_active, token_date from broker_connections
			 where user_id = $1 and broker = 'dhan'`,
			[locals.user.id]
		)
		const conn = result.rows[0]
		brokerConnected = conn?.is_active && conn?.token_date?.toISOString().slice(0, 10) === new Date().toISOString().slice(0, 10)
	}

	return { user: locals.user, brokerConnected }
}
