declare global {
	namespace App {
		interface Locals {
			user: { dhanClientId: string; name: string; tokenExpiry: string } | null
		}
	}
}

export {};
