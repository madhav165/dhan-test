import pg from 'pg'
import { DATABASE_URL } from '$env/static/private'

const pool = new pg.Pool({ connectionString: DATABASE_URL })

export const db = {
	query: (text: string, params?: unknown[]) => pool.query(text, params)
}
