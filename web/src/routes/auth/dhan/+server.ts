import { redirect } from '@sveltejs/kit'
import { DHAN_APP_ID, DHAN_APP_SECRET, DHAN_AUTH_URL } from '$env/static/private'
import { db } from '$lib/server/db'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, cookies, locals }) => {
	if (!locals.user) redirect(302, '/login')

	// Use stored client_id if already connected, otherwise read from query param
	let clientId = url.searchParams.get('client_id')?.trim()

	if (!clientId) {
		const result = await db.query(
			`select client_id from broker_connections where user_id = $1 and broker = 'dhan'`,
			[locals.user.id]
		)
		clientId = result.rows[0]?.client_id
	}

	if (!clientId || !/^\d{10}$/.test(clientId)) redirect(302, '/connect/dhan?error=invalid_client_id')

	const resp = await fetch(`${DHAN_AUTH_URL}/app/generate-consent?client_id=${clientId}`, {
		method: 'POST',
		headers: { app_id: DHAN_APP_ID, app_secret: DHAN_APP_SECRET },
	})

	if (!resp.ok) redirect(302, '/connect/dhan?error=invalid_client_id')

	const { consentAppId } = await resp.json()
	cookies.set('dhan_consent', consentAppId, { path: '/', httpOnly: true, maxAge: 600, sameSite: 'lax', secure: false })

	redirect(302, `${DHAN_AUTH_URL}/login/consentApp-login?consentAppId=${consentAppId}`)
}
