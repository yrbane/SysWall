<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
</script>

<h1 class="page-title">{fr.nav_connections}</h1>
<EmptyState message={fr.conn_no_connections} />

<style>
  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }
</style>
