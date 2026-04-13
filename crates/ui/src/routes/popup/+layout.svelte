<script lang="ts">
  import '../../app.css';
  import Toast from '$lib/components/ui/Toast.svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<!-- Layout minimal sans sidebar pour les popups -->
<!-- Minimal layout without sidebar for popups -->
<div class="popup-layout">
  {@render children()}
</div>
<Toast />

<style>
  .popup-layout {
    height: 100vh;
    overflow: hidden;
    background: var(--bg-primary);
  }
</style>
