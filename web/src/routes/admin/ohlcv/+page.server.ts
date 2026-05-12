import { error } from '@sveltejs/kit'
import { GO_URL, OHLCV_USER_ID } from '$env/static/private'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, url }) => {
	if (!locals.user || locals.user.id !== OHLCV_USER_ID) {
		error(403, 'Forbidden')
	}

	const q        = url.searchParams.get('q') ?? ''
	const industry = url.searchParams.get('industry') ?? ''
	const page     = url.searchParams.get('page') ?? '1'

	const params = new URLSearchParams({ q, industry, page })
	const resp = await fetch(`${GO_URL}/admin/ohlcv/stocks?${params}`, {
		headers: { 'X-User-ID': locals.user.id }
	})
	if (!resp.ok) error(502, 'Failed to fetch stocks')

	const data = await resp.json()

	return { ...data, q, industry }
}
