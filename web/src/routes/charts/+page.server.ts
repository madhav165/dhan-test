import { db } from '$lib/server/db'
import { fail, redirect } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals }) => {
	const result = await db.query(
		`select id, name, security_id, exchange_segment, interval, indicators
		 from charts
		 where user_id = $1
		 order by updated_at desc`,
		[locals.user!.id]
	)
	return { charts: result.rows }
}

export const actions: Actions = {
	save: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string | null
		const name = data.get('name') as string
		const security_id = data.get('security_id') as string
		const exchange_segment = data.get('exchange_segment') as string
		const interval = data.get('interval') as string
		const indicators = data.get('indicators') as string

		if (!name || !security_id || !exchange_segment || !interval) {
			return fail(400, { error: 'Missing required fields' })
		}

		let parsed
		try {
			parsed = JSON.parse(indicators || '[]')
		} catch {
			return fail(400, { error: 'Invalid indicators' })
		}

		if (id) {
			await db.query(
				`update charts set name=$1, security_id=$2, exchange_segment=$3, interval=$4, indicators=$5, updated_at=now()
				 where id=$6 and user_id=$7`,
				[name, security_id, exchange_segment, interval, JSON.stringify(parsed), id, locals.user!.id]
			)
			return { saved: true, id }
		} else {
			const r = await db.query(
				`insert into charts (user_id, name, security_id, exchange_segment, interval, indicators)
				 values ($1,$2,$3,$4,$5,$6) returning id`,
				[locals.user!.id, name, security_id, exchange_segment, interval, JSON.stringify(parsed)]
			)
			return { saved: true, id: r.rows[0].id }
		}
	},

	delete: async ({ request, locals }) => {
		const data = await request.formData()
		const id = data.get('id') as string
		await db.query(`delete from charts where id=$1 and user_id=$2`, [id, locals.user!.id])
		return { deleted: true }
	}
}
