import { db } from '$lib/server/db'
import { fail } from '@sveltejs/kit'
import type { Actions, PageServerLoad } from './$types'

export const load: PageServerLoad = async ({ locals }) => {
	const result = await db.query(
		`select telegram_chat_id from users where id = $1`,
		[locals.user!.id]
	)
	return {
		telegramConnected: !!result.rows[0]?.telegram_chat_id,
		botName: process.env.TELEGRAM_BOT_NAME ?? ''
	}
}

export const actions: Actions = {
	verify: async ({ request, locals }) => {
		const data = await request.formData()
		const token = (data.get('token') as string ?? '').trim()

		if (!/^\d{6}$/.test(token)) {
			return fail(400, { error: 'Enter the 6-digit code from the bot.' })
		}

		const row = await db.query(
			`select chat_id from telegram_link_tokens
			 where token = $1 and expires_at > now()`,
			[token]
		)

		if (!row.rows[0]) {
			return fail(400, { error: 'Invalid or expired code.' })
		}

		const chatId: string = row.rows[0].chat_id

		await db.query(`update users set telegram_chat_id = $1 where id = $2`, [chatId, locals.user!.id])
		await db.query(`delete from telegram_link_tokens where token = $1`, [token])

		return { success: true }
	},

	disconnect: async ({ locals }) => {
		await db.query(`update users set telegram_chat_id = null where id = $1`, [locals.user!.id])
		return { success: true }
	}
}
