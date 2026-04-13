<!--
  Composant de notifications toast
  Toast notification component
-->
<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import { toasts, removeToast } from '$lib/stores/toast';

  // Couleur de bordure gauche selon le variant
  // Left border color per variant
  const variantColors: Record<string, string> = {
    success: 'var(--accent-green)',
    error: 'var(--accent-red)',
    warning: 'var(--accent-orange)',
    info: 'var(--accent-cyan)',
  };
</script>

<div class="toast-container" aria-live="polite">
  {#each $toasts as toast (toast.id)}
    <div
      class="toast"
      style="border-left-color: {variantColors[toast.variant] || variantColors.info}"
      in:fly={{ y: 30, duration: 200 }}
      out:fade={{ duration: 150 }}
      role="alert"
    >
      <span class="toast-message">{toast.message}</span>
      <button
        class="toast-close"
        onclick={() => removeToast(toast.id)}
        aria-label="Fermer"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: var(--space-4);
    right: var(--space-4);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-left: 4px solid var(--accent-cyan);
    border-radius: var(--radius-md);
    min-width: 280px;
    max-width: 420px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  }

  .toast-message {
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    line-height: 1.4;
  }

  .toast-close {
    flex-shrink: 0;
    background: none;
    border: none;
    color: var(--text-tertiary);
    cursor: pointer;
    padding: var(--space-1);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast);
  }

  .toast-close:hover {
    color: var(--text-primary);
  }
</style>
