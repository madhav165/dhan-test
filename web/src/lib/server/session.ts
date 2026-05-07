import { SignJWT, jwtVerify } from 'jose'
import { SESSION_SECRET } from '$env/static/private'

const secret = new TextEncoder().encode(SESSION_SECRET)

export async function createSession(userId: string) {
	return new SignJWT({ userId })
		.setProtectedHeader({ alg: 'HS256' })
		.setExpirationTime('30d')
		.sign(secret)
}

export async function verifySession(token: string): Promise<{ userId: string } | null> {
	try {
		const { payload } = await jwtVerify(token, secret)
		return { userId: payload.userId as string }
	} catch {
		return null
	}
}
