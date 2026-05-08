import { db } from '$lib/server/db'
import { redirect, fail } from '@sveltejs/kit'
import type { Actions } from './$types'
import type { RLConfig } from '$lib/types/rl'

export const actions: Actions = {
	default: async ({ request, locals }) => {
		const form = await request.formData()
		const name = form.get('name')?.toString().trim()
		const rl_config_raw = form.get('rl_config')?.toString()

		if (!name) return fail(400, { error: 'Name is required' })
		if (!rl_config_raw) return fail(400, { error: 'RL config is required' })

		let rl_config: RLConfig
		try {
			rl_config = JSON.parse(rl_config_raw)
		} catch {
			return fail(400, { error: 'Invalid RL config' })
		}

		if (!rl_config.train_from || !rl_config.train_to) {
			return fail(400, { error: 'Training date range is required' })
		}
		if (!rl_config.test_from || !rl_config.test_to) {
			return fail(400, { error: 'Test date range is required' })
		}
		if (rl_config.test_from <= rl_config.train_to) {
			return fail(400, { error: 'Test range must start after training range ends' })
		}

		const stratResult = await db.query(
			`insert into strategies (user_id, name, strategy_type, rl_config)
			 values ($1, $2, 'rl', $3) returning id`,
			[locals.user!.id, name, rl_config]
		)
		const strategyId = stratResult.rows[0].id

		await db.query(
			`insert into rl_jobs (strategy_id) values ($1)`,
			[strategyId]
		)

		redirect(302, `/strategies/${strategyId}`)
	},
}
