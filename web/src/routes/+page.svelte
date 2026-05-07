<script lang="ts">
	import { user } from '$lib/stores/auth'
</script>

{#if $user}
	<p>Welcome, {$user.name}</p>
{/if}
