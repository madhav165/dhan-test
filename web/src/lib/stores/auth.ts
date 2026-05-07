import { writable } from 'svelte/store'

export const user = writable<{
	id: string
	email: string
	name: string
} | null>(null)

export const brokerConnected = writable<boolean>(false)
