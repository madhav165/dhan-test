import { GO_URL } from '$env/static/private'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, locals }) => {
	if (!locals.user) return new Response('Unauthorized', { status: 401 })

	const params = url.searchParams.toString()
	const resp = await fetch(`${GO_URL}/chart/candles?${params}`, {
		headers: { 'X-User-ID': locals.user.id },
	})

	const data = await resp.text()
	return new Response(data, {
		status: resp.status,
		headers: { 'Content-Type': 'application/json' },
	})
}
