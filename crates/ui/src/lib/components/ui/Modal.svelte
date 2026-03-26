<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title: string;
    size?: 'sm' | 'md' | 'lg';
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  }

  let { open, title, size = 'md', onclose, children, footer }: Props = $props();

  const sizeMap = { sm: '400px', md: '560px', lg: '720px' };

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose?.();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose?.();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="modal-backdrop"
    onclick={handleBackdrop}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <div class="modal-content" style="max-width: {sizeMap[size]};">
      <div class="modal-header">
        <h2 class="modal-title">{title}</h2>
        <button class="modal-close" onclick={onclose} aria-label="Fermer">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div class="modal-body">
        {@render children()}
      </div>
      {#if footer}
        <div class="modal-footer">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(var(--glass-blur));
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fadeIn 200ms ease;
  }

  .modal-content {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-xl);
    box-shadow: var(--glow-cyan);
    width: 90%;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    animation: slideUp 300ms ease;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-6);
    border-bottom: 1px solid var(--border-primary);
  }

  .modal-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .modal-close {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: var(--space-1);
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast);
  }

  .modal-close:hover {
    color: var(--text-primary);
  }

  .modal-body {
    padding: var(--space-6);
    overflow-y: auto;
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
    border-top: 1px solid var(--border-primary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
