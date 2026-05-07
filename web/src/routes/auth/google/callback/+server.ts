import { google } from '$lib/server/oauth'
import { db } from '$lib/server/db'
import { createSession } from '$lib/server/session'
import { redirect } from '@sveltejs/kit'
import { decodeIdToken } from 'arctic'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, cookies }) => {
	const code = url.searchParams.get('code')
	const state = url.searchParams.get('state')
	const storedState = cookies.get('oauth_state')
	const codeVerifier = cookies.get('oauth_code_verifier')

	if (!code || !state || state !== storedState || !codeVerifier) {
		redirect(302, '/login?error=oauth_failed')
	}

	const tokens = await google.validateAuthorizationCode(code, codeVerifier)
	const claims = decodeIdToken(tokens.idToken()) as { email: string; name: string }

	const result = await db.query(
		`insert into users (email, name) values ($1, $2)
		 on conflict (email) do update set name = excluded.name
		 returning id`,
		[claims.email, claims.name]
	)

	const userId = result.rows[0].id
	const session = await createSession(userId)

	cookies.set('session', session, { path: '/', httpOnly: true, maxAge: 60 * 60 * 24 * 30, sameSite: 'lax', secure: false })
	cookies.delete('oauth_state', { path: '/' })
	cookies.delete('oauth_code_verifier', { path: '/' })

	redirect(302, '/')
}
