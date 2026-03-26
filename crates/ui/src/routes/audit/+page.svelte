<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
</script>

<h1 class="page-title">{fr.audit_title}</h1>
<EmptyState message={fr.audit_no_events} />

<style>
  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }
</style>
