<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Button from './Button.svelte';

  interface Props {
    message: string;
    onretry?: () => void;
  }

  let { message, onretry }: Props = $props();
</script>

<div class="error-banner" role="alert">
  <div class="error-content">
    <svg class="error-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
    <span class="error-message">{message}</span>
  </div>
  {#if onretry}
    <Button variant="ghost" size="sm" onclick={onretry}>
      {fr.common_retry}
    </Button>
  {/if}
</div>

<style>
  .error-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    background: var(--accent-red-15);
    border: 1px solid var(--accent-red);
    border-radius: var(--radius-md);
    gap: var(--space-3);
  }

  .error-content {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .error-icon {
    color: var(--accent-red);
    flex-shrink: 0;
  }

  .error-message {
    color: var(--accent-red);
    font-size: var(--font-size-sm);
  }
</style>
