import { redirect } from '@sveltejs/kit'
import type { LayoutServerLoad } from './$types'

export const load: LayoutServerLoad = async ({ locals, url }) => {
	const isAuthRoute = url.pathname.startsWith('/auth') || url.pathname.startsWith('/login')
	if (!locals.user && !isAuthRoute) redirect(302, '/login')
	return { user: locals.user }
}
