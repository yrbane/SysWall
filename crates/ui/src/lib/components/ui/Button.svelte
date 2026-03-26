<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'success' | 'danger' | 'ghost';
    size?: 'sm' | 'md' | 'lg';
    disabled?: boolean;
    loading?: boolean;
    onclick?: () => void;
    type?: 'button' | 'submit';
    children: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    loading = false,
    onclick,
    type = 'button',
    children,
  }: Props = $props();
</script>

<button
  class="btn btn-{variant} btn-{size}"
  {type}
  disabled={disabled || loading}
  {onclick}
>
  {#if loading}
    <span class="spinner"></span>
  {/if}
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    font-family: var(--font-sans);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: all var(--transition-fast);
    white-space: nowrap;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Sizes */
  .btn-sm { padding: var(--space-1) var(--space-3); font-size: var(--font-size-xs); }
  .btn-md { padding: var(--space-2) var(--space-4); font-size: var(--font-size-sm); }
  .btn-lg { padding: var(--space-3) var(--space-6); font-size: var(--font-size-base); }

  /* Variants */
  .btn-primary {
    background: var(--accent-cyan);
    color: var(--bg-primary);
  }
  .btn-primary:hover:not(:disabled) {
    box-shadow: var(--glow-cyan);
  }

  .btn-success {
    background: var(--accent-green);
    color: var(--bg-primary);
  }
  .btn-success:hover:not(:disabled) {
    box-shadow: var(--glow-green);
  }

  .btn-danger {
    background: var(--accent-red);
    color: var(--bg-primary);
  }
  .btn-danger:hover:not(:disabled) {
    box-shadow: var(--glow-red);
  }

  .btn-ghost {
    background: transparent;
    color: var(--text-primary);
    border-color: var(--border-primary);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--accent-cyan);
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
