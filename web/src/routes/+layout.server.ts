import { redirect } from '@sveltejs/kit'
import { db } from '$lib/server/db'
import type { LayoutServerLoad } from './$types'

export const load: LayoutServerLoad = async ({ locals, url }) => {
	const isAuthRoute = url.pathname.startsWith('/auth') || url.pathname.startsWith('/login')
	if (!locals.user && !isAuthRoute) redirect(302, '/login')

	let brokerConnected = false
	if (locals.user) {
		const result = await db.query(
			`select is_active from broker_connections
			 where user_id = $1 and broker = 'dhan' and token_date = current_date`,
			[locals.user.id]
		)
		brokerConnected = result.rows[0]?.is_active === true
	}

	return { user: locals.user, brokerConnected }
}
