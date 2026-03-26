<script lang="ts">
  interface Props {
    message: string;
    icon?: string;
  }

  let { message, icon }: Props = $props();
</script>

<div class="empty-state">
  {#if icon}
    <div class="empty-icon">
      {@html icon}
    </div>
  {/if}
  <p class="empty-message">{message}</p>
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
    opacity: 0.5;
  }

  .empty-message {
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
  }
</style>
