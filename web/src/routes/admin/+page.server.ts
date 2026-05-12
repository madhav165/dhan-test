import { error } from '@sveltejs/kit'
import { GO_URL, OHLCV_USER_ID } from '$env/static/private'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user || locals.user.id !== OHLCV_USER_ID) {
		error(403, 'Forbidden')
	}

	const resp = await fetch(`${GO_URL}/admin/ohlcv`, {
		headers: { 'X-User-ID': locals.user.id }
	})
	if (!resp.ok) error(502, 'Failed to fetch stats')

	const stats = await resp.json()

	return { stats }
}
