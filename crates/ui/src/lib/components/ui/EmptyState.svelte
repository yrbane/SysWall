<!--
  Composant d'état vide avec icône, titre et description
  Empty state component with icon, title and description
-->
<script lang="ts">
  interface Props {
    /** @deprecated Utiliser title + description / Use title + description instead */
    message?: string;
    title?: string;
    description?: string;
    icon?: string;
  }

  let { message, title, description, icon }: Props = $props();

  // Icône par défaut : dossier vide / Default icon: empty folder
  const defaultIcon = '<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="9" y1="14" x2="15" y2="14"/></svg>';
</script>

<div class="empty-state">
  <div class="empty-icon">
    {@html icon || defaultIcon}
  </div>
  {#if title}
    <p class="empty-title">{title}</p>
  {/if}
  {#if description}
    <p class="empty-description">{description}</p>
  {:else if message}
    <p class="empty-description">{message}</p>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--space-12) var(--space-8);
    text-align: center;
  }

  .empty-icon {
    color: var(--text-tertiary);
    margin-bottom: var(--space-4);
    opacity: 0.4;
  }

  .empty-title {
    color: var(--text-secondary);
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    margin-bottom: var(--space-2);
  }

  .empty-description {
    color: var(--text-tertiary);
    font-size: var(--font-size-sm);
    max-width: 320px;
    line-height: 1.5;
  }
</style>
