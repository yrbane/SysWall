<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/ui/Sidebar.svelte';
  import ErrorBanner from '$lib/components/ui/ErrorBanner.svelte';
  import Toast from '$lib/components/ui/Toast.svelte';
  import { page } from '$app/stores';
  import { fade } from 'svelte/transition';
  import { fr } from '$lib/i18n/fr';
  import { firewallStatus, fetchStatus, initStatusListener, statusError } from '$lib/stores/status';
  import { initConnectionListeners, connectionCounts } from '$lib/stores/connections';
  import { fetchRules, initRuleListeners, rulesCount } from '$lib/stores/rules';
  import { fetchPendingDecisions, initDecisionListeners, pendingCount } from '$lib/stores/decisions';
  import { initAuditListener } from '$lib/stores/audit';
  import { startTrafficTrend, stopTrafficTrend } from '$lib/stores/dashboard';
  import { setNetworkEnabled } from '$lib/api/client';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  // Titres de page par route / Page titles by route
  const pageTitles: Record<string, string> = {
    '/dashboard': fr.nav_dashboard,
    '/connections': fr.nav_connections,
    '/rules': fr.nav_rules,
    '/learning': fr.nav_learning,
    '/blocklists': 'Blocklists',
    '/audit': fr.nav_audit,
    '/settings': fr.nav_settings,
  };

  const currentTitle = $derived(pageTitles[$page.url.pathname] || 'SysWall');

  const navItems = $derived([
    { label: fr.nav_dashboard, route: '/dashboard', icon: '📊' },
    { label: fr.nav_connections, route: '/connections', icon: '🔗', badge: $connectionCounts.total },
    { label: fr.nav_rules, route: '/rules', icon: '🛡️', badge: $rulesCount },
    { label: fr.nav_learning, route: '/learning', icon: '🧠', badge: $pendingCount, pulsing: $pendingCount > 0 },
    { label: 'Blocklists', route: '/blocklists', icon: '🚫' },
    { label: fr.nav_audit, route: '/audit', icon: '📋' },
    { label: fr.nav_settings, route: '/settings', icon: '⚙️' },
  ]);

  let networkEnabled = $state(true);

  // Synchronise l'état réseau avec le statut du firewall
  $effect(() => {
    networkEnabled = $firewallStatus.enabled;
  });

  async function toggleNetwork() {
    try {
      await setNetworkEnabled(!networkEnabled);
      networkEnabled = !networkEnabled;
    } catch { /* toast gère l'erreur */ }
  }

  onMount(() => {
    fetchStatus();
    fetchRules();
    fetchPendingDecisions();

    const unStatus = initStatusListener();
    const unConnections = initConnectionListeners();
    const unRules = initRuleListeners();
    const unDecisions = initDecisionListeners();
    const unAudit = initAuditListener();
    startTrafficTrend();

    return () => {
      unStatus();
      unConnections();
      unRules();
      unDecisions();
      unAudit();
      stopTrafficTrend();
    };
  });
</script>

<div class="app-layout">
  <Sidebar firewallEnabled={networkEnabled} items={navItems} />

  <div class="main-area">
    <!-- Top bar contextuelle / Contextual top bar -->
    <header class="topbar">
      <h1 class="topbar-title">{currentTitle}</h1>
      <div class="topbar-spacer"></div>

      <!-- Kill-switch pill -->
      <button class="killswitch-pill" class:disabled={!networkEnabled} onclick={toggleNetwork}>
        <span class="killswitch-dot" class:active={networkEnabled}></span>
        <span class="killswitch-label">
          {networkEnabled ? 'Réseau actif' : 'Réseau coupé'}
        </span>
      </button>
    </header>

    <main class="content">
      {#if $statusError}
        <ErrorBanner message={fr.common_connection_error} onretry={fetchStatus} />
      {/if}
      {#key $page.url.pathname}
        <div class="page-transition" in:fade={{ duration: 150, delay: 50 }} out:fade={{ duration: 100 }}>
          {@render children()}
        </div>
      {/key}
    </main>
  </div>

  <Toast />
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .main-area {
    margin-left: var(--sidebar-width);
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* --- Top bar --- */
  .topbar {
    display: flex;
    align-items: center;
    padding: 0 20px;
    height: var(--topbar-height);
    min-height: var(--topbar-height);
    background: var(--bg-topbar);
    backdrop-filter: blur(var(--glass-blur));
    border-bottom: 1px solid var(--border-primary);
    gap: 12px;
  }

  .topbar-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: -0.2px;
  }

  .topbar-spacer {
    flex: 1;
  }

  /* --- Kill-switch pill --- */
  .killswitch-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--accent-green-15);
    border: 1px solid rgba(52, 199, 89, 0.2);
    border-radius: var(--radius-pill);
    padding: 4px 12px;
    cursor: pointer;
    transition: all var(--transition-fast);
    font-family: var(--font-sans);
  }

  .killswitch-pill.disabled {
    background: var(--accent-red-15);
    border-color: rgba(255, 69, 58, 0.2);
  }

  .killswitch-pill:hover {
    filter: brightness(1.1);
  }

  .killswitch-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-red);
    transition: all var(--transition-fast);
  }

  .killswitch-dot.active {
    background: var(--accent-green);
    box-shadow: 0 0 6px rgba(52, 199, 89, 0.5);
  }

  .killswitch-label {
    font-size: 11px;
    font-weight: 500;
    color: var(--accent-green);
  }

  .killswitch-pill.disabled .killswitch-label {
    color: var(--accent-red);
  }

  /* --- Content --- */
  .content {
    flex: 1;
    padding: 20px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .page-transition {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    flex: 1;
  }

  /* --- Responsive --- */
  @media (max-width: 639px) {
    .main-area {
      margin-left: 0;
    }
    .content {
      padding-bottom: 72px;
    }
  }
</style>
