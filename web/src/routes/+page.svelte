<script lang="ts">
	import { user } from '$lib/stores/auth'
</script>

{#if $user}
	<p>Welcome, {$user.name}</p>
	<form method="POST" action="/auth/logout">
		<button type="submit">Logout</button>
	</form>
{/if}
