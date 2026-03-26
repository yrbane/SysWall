<script lang="ts">
  import { fr } from '$lib/i18n/fr';

  interface Props {
    size?: 'sm' | 'md' | 'lg';
  }

  let { size = 'md' }: Props = $props();

  const sizeMap = { sm: '16px', md: '32px', lg: '48px' };
</script>

<div class="spinner-container">
  <div
    class="spinner"
    style="width: {sizeMap[size]}; height: {sizeMap[size]};"
    role="status"
    aria-label={fr.common_loading}
  ></div>
</div>

<style>
  .spinner-container {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8);
  }

  .spinner {
    border: 3px solid var(--border-primary);
    border-top-color: var(--accent-cyan);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
