import { error } from '@sveltejs/kit'
import { GO_URL } from '$env/static/private'
import type { PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals, url }) => {
	const q        = url.searchParams.get('q') ?? ''
	const industry = url.searchParams.get('industry') ?? ''
	const page     = url.searchParams.get('page') ?? '1'

	const params = new URLSearchParams({ q, industry, page })
	const resp = await fetch(`${GO_URL}/ohlcv/stocks?${params}`, {
		headers: { 'X-User-ID': locals.user!.id }
	})
	if (!resp.ok) error(502, 'Failed to fetch stocks')

	const data = await resp.json()

	return { ...data, q, industry }
}
