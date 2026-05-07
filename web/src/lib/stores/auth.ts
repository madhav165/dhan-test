import { writable } from 'svelte/store'

export const user = writable<{
	dhanClientId: string
	name: string
	tokenExpiry: string
} | null>(null)
