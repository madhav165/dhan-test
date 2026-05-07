import { verifySession } from '$lib/server/session'
import type { Handle } from '@sveltejs/kit'

export const handle: Handle = async ({ event, resolve }) => {
	const token = event.cookies.get('session')
	const verified = token ? await verifySession(token) : null
	event.locals.user = verified ?? null
	return resolve(event)
}
