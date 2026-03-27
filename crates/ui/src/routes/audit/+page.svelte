<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import {
    auditFilters,
    auditPage,
    auditPageSize,
    paginatedAuditEvents,
    totalFilteredCount,
    totalPages,
    filteredAuditEvents,
  } from '$lib/stores/audit';

  // Local filter state bound to the store
  let searchValue = $state('');
  let severityFilter = $state('');
  let categoryFilter = $state('');
  let dateStart = $state('');
  let dateEnd = $state('');

  // Sync local state to store
  $effect(() => {
    auditFilters.set({
      search: searchValue,
      severity: severityFilter,
      category: categoryFilter,
      dateStart,
      dateEnd,
    });
    // Reset to first page on filter change
    auditPage.set(0);
  });

  function severityVariant(sev: string): 'neutral' | 'cyan' | 'orange' | 'red' | 'purple' {
    if (sev === 'debug') return 'neutral';
    if (sev === 'info') return 'cyan';
    if (sev === 'warning') return 'orange';
    if (sev === 'error') return 'red';
    if (sev === 'critical') return 'purple';
    return 'neutral';
  }

  function severityLabel(sev: string): string {
    const map: Record<string, string> = {
      debug: fr.audit_debug,
      info: fr.audit_info,
      warning: fr.audit_warning,
      error: fr.audit_error,
      critical: fr.audit_critical,
    };
    return map[sev] || sev;
  }

  function categoryVariant(cat: string): 'cyan' | 'green' | 'orange' | 'purple' | 'neutral' {
    if (cat === 'connection') return 'cyan';
    if (cat === 'rule') return 'green';
    if (cat === 'decision') return 'orange';
    if (cat === 'system') return 'purple';
    return 'neutral';
  }

  function categoryLabel(cat: string): string {
    const map: Record<string, string> = {
      connection: fr.audit_connection,
      rule: fr.audit_rule,
      decision: fr.audit_decision,
      system: fr.audit_system,
      config: fr.audit_config,
    };
    return map[cat] || cat;
  }

  // Expanded row for metadata details
  let expandedEventId = $state<string | null>(null);

  function toggleEventExpand(id: string) {
    expandedEventId = expandedEventId === id ? null : id;
  }

  // Format description: if it looks like raw JSON, try to extract a readable string
  function formatDescription(desc: string): string {
    if (!desc) return '--';
    // If the description starts with { or [, it's likely raw JSON — try to make it readable
    const trimmed = desc.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try {
        const parsed = JSON.parse(trimmed);
        // Try to extract meaningful fields
        const parts: string[] = [];
        if (parsed.process_name) parts.push(parsed.process_name);
        if (parsed.destination?.ip) parts.push(`vers ${parsed.destination.ip}${parsed.destination?.port ? ':' + parsed.destination.port : ''}`);
        if (parsed.protocol) parts.push(typeof parsed.protocol === 'string' ? parsed.protocol.toUpperCase() : '');
        if (parsed.verdict) parts.push(parsed.verdict);
        if (parsed.message) parts.push(parsed.message);
        if (parts.length > 0) return parts.filter(Boolean).join(' - ');
      } catch {
        // Not valid JSON — just use as-is
      }
    }
    return desc;
  }

  // Check if metadata has entries worth displaying
  function hasMetadata(metadata: Record<string, string> | undefined): boolean {
    if (!metadata) return false;
    return Object.keys(metadata).length > 0;
  }

  function goPage(delta: number) {
    auditPage.update((p) => {
      const next = p + delta;
      if (next < 0) return 0;
      if (next >= $totalPages) return $totalPages - 1;
      return next;
    });
  }

  // Export audit log as JSON file download
  function exportAuditLog() {
    const data = JSON.stringify($filteredAuditEvents, null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `syswall-audit-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="page-header">
  <h1 class="page-title">{fr.audit_title}</h1>
  <Button variant="ghost" size="sm" onclick={exportAuditLog}>
    {fr.audit_export}
  </Button>
</div>

<!-- Filter bar -->
<div class="filter-bar">
  <div class="filter-search">
    <Input
      type="search"
      placeholder={fr.audit_search}
      bind:value={searchValue}
    />
  </div>

  <select class="filter-select" bind:value={severityFilter}>
    <option value="">{fr.audit_filter_all} - {fr.audit_severity}</option>
    <option value="debug">{fr.audit_debug}</option>
    <option value="info">{fr.audit_info}</option>
    <option value="warning">{fr.audit_warning}</option>
    <option value="error">{fr.audit_error}</option>
    <option value="critical">{fr.audit_critical}</option>
  </select>

  <select class="filter-select" bind:value={categoryFilter}>
    <option value="">{fr.audit_filter_all} - {fr.audit_category}</option>
    <option value="connection">{fr.audit_connection}</option>
    <option value="rule">{fr.audit_rule}</option>
    <option value="decision">{fr.audit_decision}</option>
    <option value="system">{fr.audit_system}</option>
    <option value="config">{fr.audit_config}</option>
  </select>

  <div class="date-range">
    <Input type="date" label={fr.audit_from} bind:value={dateStart} />
  </div>
  <div class="date-range">
    <Input type="date" label={fr.audit_to} bind:value={dateEnd} />
  </div>
</div>

<!-- Audit table -->
{#if $paginatedAuditEvents.length > 0}
  <div class="audit-table">
    <div class="table-header">
      <div class="col col-timestamp">{fr.audit_timestamp}</div>
      <div class="col col-severity">{fr.audit_severity}</div>
      <div class="col col-category">{fr.audit_category}</div>
      <div class="col col-description">{fr.audit_description}</div>
      <div class="col col-metadata">{fr.audit_metadata}</div>
    </div>
    <div class="table-body">
      {#each $paginatedAuditEvents as event (event.id)}
        <div
          class="table-row"
          class:expanded={expandedEventId === event.id}
          onclick={() => hasMetadata(event.metadata) && toggleEventExpand(event.id)}
          role={hasMetadata(event.metadata) ? 'button' : undefined}
          tabindex={hasMetadata(event.metadata) ? 0 : undefined}
          onkeydown={(e) => e.key === 'Enter' && hasMetadata(event.metadata) && toggleEventExpand(event.id)}
        >
          <div class="col col-timestamp font-mono">
            {new Date(event.timestamp).toLocaleString('fr-FR')}
          </div>
          <div class="col col-severity">
            <Badge variant={severityVariant(event.severity)} label={severityLabel(event.severity)} />
          </div>
          <div class="col col-category">
            <Badge variant={categoryVariant(event.category)} label={categoryLabel(event.category)} />
          </div>
          <div class="col col-description truncate" title={formatDescription(event.description)}>
            {formatDescription(event.description)}
          </div>
          <div class="col col-metadata">
            {#if hasMetadata(event.metadata)}
              <span class="metadata-count text-xs text-secondary font-mono">
                {Object.keys(event.metadata).length}
              </span>
            {:else}
              <span class="text-tertiary">--</span>
            {/if}
          </div>
        </div>
        <!-- Expanded metadata panel -->
        {#if expandedEventId === event.id && hasMetadata(event.metadata)}
          <div class="metadata-panel">
            <div class="metadata-badges">
              {#each Object.entries(event.metadata) as [key, value]}
                <span class="metadata-badge" title="{key}={value}">
                  <span class="metadata-key">{key}</span>
                  <span class="metadata-val">{value}</span>
                </span>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  </div>

  <!-- Pagination -->
  <div class="pagination">
    <Button
      variant="ghost"
      size="sm"
      disabled={$auditPage <= 0}
      onclick={() => goPage(-1)}
    >
      {fr.audit_previous}
    </Button>
    <span class="page-info text-sm text-secondary font-mono">
      {fr.audit_page} {$auditPage + 1} / {$totalPages}
      ({$totalFilteredCount} {fr.audit_items_per_page})
    </span>
    <Button
      variant="ghost"
      size="sm"
      disabled={$auditPage >= $totalPages - 1}
      onclick={() => goPage(1)}
    >
      {fr.audit_next}
    </Button>
  </div>
{:else}
  <EmptyState message={fr.audit_no_events} />
{/if}

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

  .filter-bar {
    display: flex;
    align-items: flex-end;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .filter-search {
    flex: 1;
    min-width: 200px;
    max-width: 300px;
  }

  .filter-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    cursor: pointer;
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .filter-select:focus {
    border-color: var(--accent-cyan);
  }

  .filter-select option {
    background: var(--bg-secondary);
    color: var(--text-primary);
  }

  .date-range {
    min-width: 140px;
  }

  /* Audit table */
  .audit-table {
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .table-header {
    display: flex;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    padding: var(--space-2) var(--space-4);
  }

  .table-header .col {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .table-body {
    max-height: calc(100vh - 380px);
    overflow-y: auto;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
    transition: background var(--transition-fast);
  }

  .table-row:last-child {
    border-bottom: none;
  }

  .table-row:hover {
    background: var(--bg-hover);
  }

  .col {
    font-size: var(--font-size-sm);
  }

  .col-timestamp {
    width: 180px;
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .col-severity {
    width: 120px;
    flex-shrink: 0;
  }

  .col-category {
    width: 120px;
    flex-shrink: 0;
  }

  .col-description {
    flex: 1;
    min-width: 0;
  }

  .col-metadata {
    width: 80px;
    flex-shrink: 0;
    text-align: center;
  }

  .metadata-count {
    cursor: pointer;
  }

  .table-row.expanded {
    background: var(--bg-hover);
    border-bottom-color: var(--accent-cyan);
  }

  .table-row[role='button'] {
    cursor: pointer;
  }

  .metadata-panel {
    padding: var(--space-3) var(--space-4);
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    animation: slideDown 200ms ease;
  }

  @keyframes slideDown {
    from { opacity: 0; max-height: 0; }
    to { opacity: 1; max-height: 300px; }
  }

  .metadata-badges {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .metadata-badge {
    display: inline-flex;
    align-items: center;
    font-size: var(--font-size-xs);
    border-radius: var(--radius-md);
    overflow: hidden;
    border: 1px solid var(--border-primary);
  }

  .metadata-key {
    padding: 2px 6px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-mono);
  }

  .metadata-val {
    padding: 2px 6px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-family: var(--font-mono);
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Pagination */
  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
  }

  .page-info {
    white-space: nowrap;
  }
</style>
