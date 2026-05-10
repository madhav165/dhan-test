import pg from 'pg'
import { DATABASE_URL } from '$env/static/private'

// Return DATE columns as plain YYYY-MM-DD strings to avoid timezone skew
pg.types.setTypeParser(pg.types.builtins.DATE, (val) => val)

const pool = new pg.Pool({ connectionString: DATABASE_URL })

export const db = {
	query: (text: string, params?: unknown[]) => pool.query(text, params)
}
