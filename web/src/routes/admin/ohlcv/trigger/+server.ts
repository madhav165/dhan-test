import { GO_URL, OHLCV_USER_ID } from '$env/static/private'
import { error } from '@sveltejs/kit'

export async function POST({ locals }: { locals: any }) {
	if (!locals.user || locals.user.id !== OHLCV_USER_ID) {
		error(403, 'Forbidden')
	}

	const resp = await fetch(`${GO_URL}/internal/ohlcv-trigger`, {
		method: 'POST',
		headers: { 'X-User-ID': locals.user.id }
	})

	if (!resp.ok && resp.status !== 204) {
		error(502, 'Failed to trigger OHLCV jobs')
	}
}
