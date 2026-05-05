<script lang="ts">
  import { readIcon } from '$lib/api/client';
  import { onMount } from 'svelte';

  interface Props {
    /** Chemin vers le fichier icône (ex: /usr/share/icons/Papirus/48x48/apps/firefox.svg) */
    path?: string;
    size?: number;
  }

  let { path, size = 20 }: Props = $props();

  let dataUri = $state<string | null>(null);
  let failed = $state(false);

  // Cache global des data URIs pour éviter les appels répétés
  // Global data URI cache to avoid repeated calls
  const iconCache = (globalThis as any).__syswall_icon_cache ??= new Map<string, string>();

  $effect(() => {
    if (!path) {
      dataUri = null;
      failed = false;
      return;
    }

    const cached = iconCache.get(path);
    if (cached) {
      dataUri = cached;
      return;
    }

    failed = false;
    dataUri = null;

    readIcon(path).then((uri) => {
      dataUri = uri;
      iconCache.set(path!, uri);
    }).catch(() => {
      failed = true;
    });
  });
</script>

{#if dataUri && !failed}
  <img
    src={dataUri}
    alt=""
    width={size}
    height={size}
    class="app-icon"
    onerror={() => { failed = true; }}
  />
{:else}
  <!-- Icône par défaut / Default icon -->
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--text-disabled)" stroke-width="1.5" class="app-icon">
    <rect x="2" y="3" width="20" height="14" rx="2" />
    <line x1="8" y1="21" x2="16" y2="21" />
    <line x1="12" y1="17" x2="12" y2="21" />
  </svg>
{/if}

<style>
  .app-icon {
    flex-shrink: 0;
    border-radius: 3px;
    object-fit: contain;
  }
</style>
