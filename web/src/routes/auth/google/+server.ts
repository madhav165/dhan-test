import { google } from '$lib/server/oauth'
import { redirect } from '@sveltejs/kit'
import { generateState, generateCodeVerifier } from 'arctic'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ cookies }) => {
	const state = generateState()
	const codeVerifier = generateCodeVerifier()
	const url = google.createAuthorizationURL(state, codeVerifier, ['openid', 'email', 'profile'])

	cookies.set('oauth_state', state, { path: '/', httpOnly: true, maxAge: 600, sameSite: 'lax', secure: false })
	cookies.set('oauth_code_verifier', codeVerifier, { path: '/', httpOnly: true, maxAge: 600, sameSite: 'lax', secure: false })

	redirect(302, url.toString())
}
