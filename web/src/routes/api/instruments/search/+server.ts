import { GO_URL } from '$env/static/private'
import type { RequestHandler } from './$types'

export const GET: RequestHandler = async ({ url, locals }) => {
	if (!locals.user) return new Response('Unauthorized', { status: 401 })

	const q = url.searchParams.get('q') ?? ''
	const resp = await fetch(`${GO_URL}/instruments/search?q=${encodeURIComponent(q)}`)
	const data = await resp.text()

	return new Response(data, {
		status: resp.status,
		headers: { 'Content-Type': 'application/json' },
	})
}
