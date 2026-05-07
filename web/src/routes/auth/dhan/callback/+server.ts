import { redirect } from '@sveltejs/kit'
import { DHAN_APP_ID, DHAN_APP_SECRET, DHAN_AUTH_URL } from '$env/static/private'
import { createSession } from '$lib/server/session'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, cookies }) => {
	const tokenId = url.searchParams.get('tokenId')
	const storedConsent = cookies.get('dhan_consent')

	if (!tokenId) return new Response('Missing tokenId', { status: 400 })
	if (!storedConsent) return new Response('Invalid session', { status: 400 })

	const resp = await fetch(`${DHAN_AUTH_URL}/app/consumeApp-consent?tokenId=${tokenId}`, {
		method: 'POST',
		headers: { app_id: DHAN_APP_ID, app_secret: DHAN_APP_SECRET },
	})

	if (!resp.ok) return new Response('Failed to consume consent', { status: 400 })

	const { dhanClientId, dhanClientName, accessToken, expiryTime } = await resp.json()

	const session = await createSession(dhanClientId, dhanClientName, expiryTime)
	cookies.set('session', session, { path: '/', httpOnly: true, maxAge: 60 * 60 * 24 * 30, sameSite: 'lax', secure: false })
	cookies.delete('dhan_consent', { path: '/' })

	redirect(302, '/')
}
