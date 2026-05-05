<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title?: string;
    padding?: 'sm' | 'md' | 'lg';
    children: Snippet;
  }

  let { title, padding = 'md', children }: Props = $props();

  const paddingMap = { sm: 'var(--space-3)', md: 'var(--space-4)', lg: 'var(--space-6)' };
</script>

<div
  class="card"
  style="padding: {paddingMap[padding]};"
>
  {#if title}
    <h3 class="card-title">{title}</h3>
  {/if}
  <div class="card-body">
    {@render children()}
  </div>
</div>

<style>
  .card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    transition: box-shadow var(--transition-base);
  }

  .card-title {
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: var(--space-3);
  }

  .card-body {
    width: 100%;
  }
</style>
