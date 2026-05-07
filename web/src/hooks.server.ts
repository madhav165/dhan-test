import { verifySession } from '$lib/server/session'
import { db } from '$lib/server/db'
import type { Handle } from '@sveltejs/kit'

export const handle: Handle = async ({ event, resolve }) => {
	const token = event.cookies.get('session')
	const verified = token ? await verifySession(token) : null

	if (verified) {
		const result = await db.query(
			`select id, email, name from users where id = $1`,
			[verified.userId]
		)
		event.locals.user = result.rows[0] ?? null
	} else {
		event.locals.user = null
	}

	return resolve(event)
}
