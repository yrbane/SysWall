<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import StatCard from '$lib/components/ui/StatCard.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import {
    dashboardSummary,
    topApps,
    topDestinations,
    recentAlerts,
  } from '$lib/stores/dashboard';
  import { connectionCounts } from '$lib/stores/connections';
  import { fetchStatus, firewallStatus } from '$lib/stores/status';
  import { onMount, onDestroy } from 'svelte';

  let refreshing = $state(false);
  let lastRefresh = $state(new Date());

  // Format uptime from seconds to human-readable string
  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  }

  async function handleRefresh() {
    refreshing = true;
    try {
      await fetchStatus();
      lastRefresh = new Date();
    } finally {
      refreshing = false;
    }
  }

  // Auto-refresh status every 30 seconds
  let autoRefreshInterval: ReturnType<typeof setInterval>;
  onMount(() => {
    autoRefreshInterval = setInterval(() => {
      fetchStatus();
      lastRefresh = new Date();
    }, 30000);
  });

  onDestroy(() => {
    clearInterval(autoRefreshInterval);
  });

  // Severity color mapping for alerts
  function severityVariant(sev: string): 'red' | 'orange' | 'purple' | 'neutral' {
    if (sev === 'critical') return 'purple';
    if (sev === 'error') return 'red';
    if (sev === 'warning') return 'orange';
    return 'neutral';
  }
</script>

<div class="page-header">
  <h1 class="page-title">{fr.nav_dashboard}</h1>
  <div class="header-actions">
    <span class="auto-refresh-label text-secondary text-xs">
      {fr.dash_auto_refresh}
    </span>
    <Button variant="ghost" size="sm" loading={refreshing} onclick={handleRefresh}>
      {fr.dash_refresh}
    </Button>
  </div>
</div>

<!-- Stat cards row -->
<div class="stats-grid">
  <StatCard
    label={fr.dash_active_connections}
    value={$dashboardSummary.activeConnections}
    color="cyan"
  />
  <StatCard
    label={fr.dash_blocked}
    value={$dashboardSummary.blocked}
    color="red"
  />
  <StatCard
    label={fr.dash_allowed}
    value={$dashboardSummary.allowed}
    color="green"
  />
  <StatCard
    label={fr.dash_pending}
    value={$connectionCounts.pending}
    color="orange"
  />
</div>

<!-- Main dashboard grid -->
<div class="dashboard-grid">
  <!-- Top applications -->
  <Card title={fr.dash_top_apps}>
    {#if $topApps.length > 0}
      {#each $topApps as app, i}
        <div class="top-item">
          <span class="rank font-mono">{i + 1}</span>
          <span class="top-item-name truncate">{app.name}</span>
          <span class="top-item-count font-mono text-cyan">{app.count}</span>
        </div>
      {/each}
    {:else}
      <p class="text-secondary text-sm">{fr.dash_waiting}</p>
    {/if}
  </Card>

  <!-- Top destinations -->
  <Card title={fr.dash_top_destinations}>
    {#if $topDestinations.length > 0}
      {#each $topDestinations as dest, i}
        <div class="top-item">
          <span class="rank font-mono">{i + 1}</span>
          <span class="top-item-name truncate font-mono">{dest.ip}</span>
          <span class="top-item-count font-mono text-cyan">{dest.count}</span>
        </div>
      {/each}
    {:else}
      <p class="text-secondary text-sm">{fr.dash_waiting}</p>
    {/if}
  </Card>

  <!-- Recent alerts -->
  <Card title={fr.dash_recent_alerts}>
    {#if $recentAlerts.length > 0}
      <div class="alerts-list">
        {#each $recentAlerts.slice(0, 10) as alert}
          <div class="alert-item">
            <Badge variant={severityVariant(alert.severity)} label={alert.severity} />
            <span class="alert-desc text-sm truncate">{alert.description}</span>
            <span class="alert-time text-tertiary text-xs font-mono">
              {new Date(alert.timestamp).toLocaleTimeString('fr-FR')}
            </span>
          </div>
        {/each}
      </div>
    {:else}
      <p class="text-secondary text-sm">--</p>
    {/if}
  </Card>

  <!-- Firewall status card -->
  <Card title={fr.dash_firewall_status}>
    <div class="status-grid">
      <div class="status-row">
        <span class="status-key text-secondary text-sm">{fr.settings_status}</span>
        <Badge
          variant={$firewallStatus.enabled ? 'green' : 'red'}
          label={$firewallStatus.enabled ? fr.status_active : fr.status_inactive}
          dot
        />
      </div>
      <div class="status-row">
        <span class="status-key text-secondary text-sm">{fr.dash_version}</span>
        <span class="font-mono text-sm">{$firewallStatus.version || '--'}</span>
      </div>
      <div class="status-row">
        <span class="status-key text-secondary text-sm">{fr.dash_uptime}</span>
        <span class="font-mono text-sm text-cyan">{formatUptime($firewallStatus.uptime_secs)}</span>
      </div>
      <div class="status-row">
        <span class="status-key text-secondary text-sm">{fr.dash_nftables}</span>
        <Badge
          variant={$firewallStatus.nftables_synced ? 'green' : 'orange'}
          label={$firewallStatus.nftables_synced ? fr.status_synced : fr.status_not_synced}
          dot
        />
      </div>
    </div>
  </Card>
</div>

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .auto-refresh-label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-4);
  }

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-4);
  }

  .top-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .top-item:last-child {
    border-bottom: none;
  }

  .rank {
    color: var(--text-tertiary);
    font-size: var(--font-size-xs);
    width: 1.5em;
    flex-shrink: 0;
  }

  .top-item-name {
    flex: 1;
    min-width: 0;
  }

  .top-item-count {
    flex-shrink: 0;
  }

  .alerts-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    max-height: 320px;
    overflow-y: auto;
  }

  .alert-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .alert-item:last-child {
    border-bottom: none;
  }

  .alert-desc {
    flex: 1;
    min-width: 0;
  }

  .alert-time {
    flex-shrink: 0;
  }

  .status-grid {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .status-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .status-row:last-child {
    border-bottom: none;
  }

  .status-key {
    flex-shrink: 0;
  }
</style>
