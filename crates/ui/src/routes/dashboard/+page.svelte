<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import StatCard from '$lib/components/ui/StatCard.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import { dashboardSummary, topApps, topDestinations, trafficTrend, recentAlerts } from '$lib/stores/dashboard';
</script>

<h1 class="page-title">{fr.nav_dashboard}</h1>

<div class="stats-grid">
  <StatCard
    label={fr.dash_active_connections}
    value={$dashboardSummary.activeConnections}
    color="cyan"
  />
  <StatCard
    label={fr.dash_allowed}
    value={$dashboardSummary.allowed}
    color="green"
  />
  <StatCard
    label={fr.dash_blocked}
    value={$dashboardSummary.blocked}
    color="red"
  />
  <StatCard
    label={fr.dash_alerts}
    value={$recentAlerts.length}
    color="orange"
  />
</div>

<div class="dashboard-grid">
  <Card title={fr.dash_traffic_trend}>
    <p class="text-secondary text-sm">{fr.dash_waiting}</p>
  </Card>

  <Card title={fr.dash_top_apps}>
    {#if $topApps.length > 0}
      {#each $topApps as app, i}
        <div class="top-item">
          <span class="rank font-mono">{i + 1}</span>
          <span class="truncate">{app.name}</span>
          <span class="font-mono text-cyan">{app.count}</span>
        </div>
      {/each}
    {:else}
      <p class="text-secondary text-sm">--</p>
    {/if}
  </Card>

  <Card title={fr.dash_top_destinations}>
    {#if $topDestinations.length > 0}
      {#each $topDestinations as dest, i}
        <div class="top-item">
          <span class="rank font-mono">{i + 1}</span>
          <span class="truncate font-mono">{dest.ip}</span>
          <span class="font-mono text-cyan">{dest.count}</span>
        </div>
      {/each}
    {:else}
      <p class="text-secondary text-sm">--</p>
    {/if}
  </Card>

  <Card title={fr.dash_recent_alerts}>
    {#if $recentAlerts.length > 0}
      {#each $recentAlerts as alert}
        <div class="alert-item">
          <span class="text-orange text-xs font-mono">{alert.severity}</span>
          <span class="text-sm truncate">{alert.description}</span>
        </div>
      {/each}
    {:else}
      <p class="text-secondary text-sm">--</p>
    {/if}
  </Card>
</div>

<style>
  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
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
</style>
