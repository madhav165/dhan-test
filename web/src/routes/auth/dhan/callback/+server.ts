import { redirect } from '@sveltejs/kit'
import { DHAN_APP_ID, DHAN_APP_SECRET, DHAN_AUTH_URL } from '$env/static/private'
import { db } from '$lib/server/db'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, cookies, locals }) => {
	const tokenId = url.searchParams.get('tokenId')
	const storedConsent = cookies.get('dhan_consent')

	if (!tokenId) return new Response('Missing tokenId', { status: 400 })
	if (!storedConsent) return new Response('Invalid session', { status: 400 })
	if (!locals.user) redirect(302, '/login')

	const resp = await fetch(`${DHAN_AUTH_URL}/app/consumeApp-consent?tokenId=${tokenId}`, {
		method: 'POST',
		headers: { app_id: DHAN_APP_ID, app_secret: DHAN_APP_SECRET },
	})

	if (!resp.ok) redirect(302, '/?error=dhan_auth_failed')

	const { dhanClientId, accessToken, expiryTime } = await resp.json()

	// Store broker connection — token encryption will be added with Go service
	await db.query(
		`insert into broker_connections (user_id, broker, client_id, encrypted_token, token_date, is_active)
		 values ($1, 'dhan', $2, $3, current_date, true)
		 on conflict (user_id, broker) do update
		 set client_id = excluded.client_id,
		     encrypted_token = excluded.encrypted_token,
		     token_date = excluded.token_date,
		     is_active = true`,
		[locals.user.id, dhanClientId, accessToken]
	)

	cookies.delete('dhan_consent', { path: '/' })
	redirect(302, '/')
}
