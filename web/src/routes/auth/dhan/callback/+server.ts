import { redirect } from '@sveltejs/kit'
import { DHAN_APP_ID, DHAN_APP_SECRET, DHAN_AUTH_URL, GO_URL, INTERNAL_SECRET } from '$env/static/private'
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

	const { dhanClientId, accessToken } = await resp.json()

	const goResp = await fetch(`${GO_URL}/internal/broker-token`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			'X-Internal-Secret': INTERNAL_SECRET,
		},
		body: JSON.stringify({
			user_id: locals.user.id,
			broker: 'dhan',
			client_id: dhanClientId,
			token: accessToken,
		}),
	})

	if (!goResp.ok) redirect(302, '/?error=dhan_auth_failed')

	cookies.delete('dhan_consent', { path: '/' })
	redirect(302, '/')
}
