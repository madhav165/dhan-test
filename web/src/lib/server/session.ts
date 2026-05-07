import { SignJWT, jwtVerify } from 'jose'
import { SESSION_SECRET } from '$env/static/private'

const secret = new TextEncoder().encode(SESSION_SECRET)

export async function createSession(dhanClientId: string, name: string, tokenExpiry: string) {
	return new SignJWT({ dhanClientId, name, tokenExpiry })
		.setProtectedHeader({ alg: 'HS256' })
		.setExpirationTime('30d')
		.sign(secret)
}

export async function verifySession(token: string): Promise<{ dhanClientId: string; name: string; tokenExpiry: string } | null> {
	try {
		const { payload } = await jwtVerify(token, secret)
		return {
			dhanClientId: payload.dhanClientId as string,
			name: payload.name as string,
			tokenExpiry: payload.tokenExpiry as string,
		}
	} catch {
		return null
	}
}
