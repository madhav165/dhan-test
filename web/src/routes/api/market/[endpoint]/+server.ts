import { GO_URL } from '$env/static/private'
import type { RequestHandler } from './$types'

export const POST: RequestHandler = async ({ params, request, locals }) => {
	if (!locals.user) return new Response('Unauthorized', { status: 401 })

	const allowed = ['ltp', 'ohlc', 'quote']
	if (!allowed.includes(params.endpoint)) {
		return new Response('Not found', { status: 404 })
	}

	const body = await request.text()

	const resp = await fetch(`${GO_URL}/market/${params.endpoint}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			'X-User-ID': locals.user.id,
		},
		body,
	})

	const data = await resp.text()
	return new Response(data, {
		status: resp.status,
		headers: { 'Content-Type': 'application/json' },
	})
}
