import { error } from '@sveltejs/kit'
import { GO_URL, ENCRYPTION_KEY, OHLCV_USER_ID } from '$env/static/private'
import { createHmac } from 'crypto'
import type { PageServerLoad } from './$types'

function makeWsToken(userId: string): string {
	const expiry = Math.floor(Date.now() / 1000) + 5 * 60
	const payload = Buffer.from(JSON.stringify({ u: userId, x: expiry })).toString('base64url')
	const sig = createHmac('sha256', Buffer.from(ENCRYPTION_KEY, 'hex')).update(payload).digest('hex')
	return `${payload}.${sig}`
}

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user || locals.user.id !== OHLCV_USER_ID) {
		error(403, 'Forbidden')
	}

	const resp = await fetch(`${GO_URL}/admin/ohlcv`, {
		headers: { 'X-User-ID': locals.user.id }
	})
	if (!resp.ok) error(502, 'Failed to fetch stats')

	const stats = await resp.json()
	const goWsUrl = GO_URL.replace(/^http/, 'ws')
	const wsToken = makeWsToken(locals.user.id)

	return { stats, goWsUrl, wsToken }
}
