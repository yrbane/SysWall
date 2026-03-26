<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import { firewallStatus } from '$lib/stores/status';

  // Format uptime from seconds to human-readable string
  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs} ${fr.common_seconds}`;
    if (secs < 3600) {
      const m = Math.floor(secs / 60);
      const s = secs % 60;
      return `${m} ${fr.common_minutes} ${s} ${fr.common_seconds}`;
    }
    if (secs < 86400) {
      const h = Math.floor(secs / 3600);
      const m = Math.floor((secs % 3600) / 60);
      return `${h} ${fr.common_hours} ${m} ${fr.common_minutes}`;
    }
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    return `${d} ${fr.common_days} ${h} ${fr.common_hours}`;
  }
</script>

<h1 class="page-title">{fr.settings_title}</h1>
<p class="settings-note text-sm text-secondary">{fr.settings_config_readonly}</p>

<div class="settings-grid">
  <!-- Firewall section -->
  <Card title={fr.settings_firewall}>
    <div class="config-list">
      <div class="config-row">
        <span class="config-key">{fr.settings_status}</span>
        <span class="config-value">
          <Badge
            variant={$firewallStatus.enabled ? 'green' : 'red'}
            label={$firewallStatus.enabled ? fr.status_active : fr.status_inactive}
            dot
          />
        </span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_default_policy}</span>
        <span class="config-value font-mono">block</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_nftables_table}</span>
        <span class="config-value font-mono">syswall</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.dash_nftables}</span>
        <span class="config-value">
          <Badge
            variant={$firewallStatus.nftables_synced ? 'green' : 'orange'}
            label={$firewallStatus.nftables_synced ? fr.status_synced : fr.status_not_synced}
            dot
          />
        </span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_rollback_timeout}</span>
        <span class="config-value font-mono">30s</span>
      </div>
    </div>
  </Card>

  <!-- Monitoring section -->
  <Card title={fr.settings_monitoring}>
    <div class="config-list">
      <div class="config-row">
        <span class="config-key">{fr.settings_conntrack_buffer}</span>
        <span class="config-value font-mono">4096</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_process_cache_ttl}</span>
        <span class="config-value font-mono">60s</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_event_bus_capacity}</span>
        <span class="config-value font-mono">1024</span>
      </div>
    </div>
  </Card>

  <!-- Learning section -->
  <Card title={fr.settings_learning}>
    <div class="config-list">
      <div class="config-row">
        <span class="config-key">{fr.settings_enabled}</span>
        <span class="config-value">
          <Badge variant="green" label={fr.settings_enabled} dot />
        </span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_debounce_window}</span>
        <span class="config-value font-mono">2s</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_prompt_timeout}</span>
        <span class="config-value font-mono">30s</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_max_pending}</span>
        <span class="config-value font-mono">10</span>
      </div>
    </div>
  </Card>

  <!-- About section -->
  <Card title={fr.settings_about}>
    <div class="config-list">
      <div class="config-row">
        <span class="config-key">{fr.settings_version}</span>
        <span class="config-value font-mono text-cyan">{$firewallStatus.version || '--'}</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_uptime}</span>
        <span class="config-value font-mono">{formatUptime($firewallStatus.uptime_secs)}</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_socket}</span>
        <span class="config-value font-mono">/var/run/syswall/syswall.sock</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_theme}</span>
        <span class="config-value">{fr.settings_theme_dark}</span>
      </div>
      <div class="config-row">
        <span class="config-key">{fr.settings_locale}</span>
        <span class="config-value">{fr.settings_locale_fr}</span>
      </div>
    </div>
  </Card>
</div>

<style>
  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .settings-note {
    margin-top: calc(-1 * var(--space-4));
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-4);
  }

  .config-list {
    display: flex;
    flex-direction: column;
  }

  .config-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .config-row:last-child {
    border-bottom: none;
  }

  .config-key {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }

  .config-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }
</style>
