<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/ui/Sidebar.svelte';
  import ErrorBanner from '$lib/components/ui/ErrorBanner.svelte';
  import { fr } from '$lib/i18n/fr';
  import { firewallStatus, fetchStatus, initStatusListener, statusError } from '$lib/stores/status';
  import { initConnectionListeners, connectionCounts } from '$lib/stores/connections';
  import { fetchRules, initRuleListeners, rulesCount } from '$lib/stores/rules';
  import { fetchPendingDecisions, initDecisionListeners, pendingCount } from '$lib/stores/decisions';
  import { initAuditListener } from '$lib/stores/audit';
  import { startTrafficTrend, stopTrafficTrend } from '$lib/stores/dashboard';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  // SVG icons for sidebar navigation
  const icons = {
    grid: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>',
    activity: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>',
    shield: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>',
    brain: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a7 7 0 0 0-7 7c0 3 2 5.5 4 7l3 3 3-3c2-1.5 4-4 4-7a7 7 0 0 0-7-7z"/><circle cx="12" cy="10" r="2"/></svg>',
    scroll: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/><polyline points="14 2 14 8 20 8"/><line x1="8" y1="13" x2="16" y2="13"/><line x1="8" y1="17" x2="16" y2="17"/></svg>',
    settings: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>',
  };

  const navItems = $derived([
    { label: fr.nav_dashboard, route: '/dashboard', icon: icons.grid },
    { label: fr.nav_connections, route: '/connections', icon: icons.activity, badge: $connectionCounts.total },
    { label: fr.nav_rules, route: '/rules', icon: icons.shield, badge: $rulesCount },
    { label: fr.nav_learning, route: '/learning', icon: icons.brain, badge: $pendingCount, pulsing: $pendingCount > 0 },
    { label: fr.nav_audit, route: '/audit', icon: icons.scroll },
    { label: fr.nav_settings, route: '/settings', icon: icons.settings },
  ]);

  onMount(() => {
    // Fetch initial data
    fetchStatus();
    fetchRules();
    fetchPendingDecisions();

    // Subscribe to real-time events
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
  <Sidebar firewallEnabled={$firewallStatus.enabled} items={navItems} />

  <main class="content">
    {#if $statusError}
      <ErrorBanner message={fr.common_connection_error} onretry={fetchStatus} />
    {/if}
    {@render children()}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .content {
    margin-left: var(--sidebar-width);
    flex: 1;
    padding: var(--space-8);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }
</style>
