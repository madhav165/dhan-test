import { redirect } from '@sveltejs/kit'
import { DHAN_APP_ID, DHAN_APP_SECRET, DHAN_AUTH_URL } from '$env/static/private'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, cookies }) => {
	const clientId = url.searchParams.get('client_id')?.trim()
	if (!clientId || !/^\d{10}$/.test(clientId)) redirect(302, '/login?error=invalid_client_id')

	const resp = await fetch(`${DHAN_AUTH_URL}/app/generate-consent?client_id=${clientId}`, {
		method: 'POST',
		headers: { app_id: DHAN_APP_ID, app_secret: DHAN_APP_SECRET },
	})

	if (!resp.ok) throw new Error(`Failed to generate consent: ${resp.status}`)

	const { consentAppId } = await resp.json()
	cookies.set('dhan_consent', consentAppId, { path: '/', httpOnly: true, maxAge: 600, sameSite: 'lax', secure: false })

	redirect(302, `${DHAN_AUTH_URL}/login/consentApp-login?consentAppId=${consentAppId}`)
}
