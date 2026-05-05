<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Table from '$lib/components/ui/Table.svelte';
  import { debounce } from '$lib/utils/debounce';
  import {
    auditFilters,
    filteredAuditEvents,
  } from '$lib/stores/audit';

  // Saisie locale non-débouncée / raw input before debounce
  let searchInput = $state('');
  let debouncedSearch = $state('');
  let severityFilter = $state('');
  let categoryFilter = $state('');
  let dateStart = $state('');
  let dateEnd = $state('');

  // Debounce de 250 ms sur la saisie de recherche
  const updateSearch = debounce((v: string) => { debouncedSearch = v; }, 250);
  $effect(() => { updateSearch(searchInput); });

  // Synchronise les filtres vers le store à chaque changement
  $effect(() => {
    auditFilters.set({
      search: debouncedSearch,
      severity: severityFilter,
      category: categoryFilter,
      dateStart,
      dateEnd,
    });
  });

  // Colonnes pour Table.svelte / Columns for Table.svelte
  const columns = [
    { key: 'timestamp_fmt', label: fr.audit_timestamp, width: '180px', mono: true },
    { key: 'severity', label: fr.audit_severity, width: '100px' },
    { key: 'category', label: fr.audit_category, width: '120px' },
    { key: 'description_fmt', label: fr.audit_description },
  ];

  // Lignes formatées pour Table.svelte / Formatted rows for Table.svelte
  const tableRows = $derived(
    $filteredAuditEvents.map((event) => ({
      ...event,
      timestamp_fmt: new Date(event.timestamp).toLocaleString('fr-FR'),
      description_fmt: formatDescription(event.description),
    }))
  );

  function formatDescription(desc: string): string {
    if (!desc) return '--';
    const trimmed = desc.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try {
        const parsed = JSON.parse(trimmed);
        const parts: string[] = [];
        if (parsed.process_name) parts.push(parsed.process_name);
        if (parsed.destination?.ip) parts.push(`vers ${parsed.destination.ip}${parsed.destination?.port ? ':' + parsed.destination.port : ''}`);
        if (parsed.protocol) parts.push(typeof parsed.protocol === 'string' ? parsed.protocol.toUpperCase() : '');
        if (parsed.verdict) parts.push(parsed.verdict);
        if (parsed.message) parts.push(parsed.message);
        if (parts.length > 0) return parts.filter(Boolean).join(' - ');
      } catch {
        // Non-JSON — utilisation brute
      }
    }
    return desc;
  }

  // Export audit log comme fichier JSON
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

<!-- Barre de filtres / Filter bar -->
<div class="filter-bar">
  <div class="filter-search">
    <Input
      type="search"
      placeholder={fr.audit_search}
      bind:value={searchInput}
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

<!-- Tableau virtualisé / Virtualized table -->
{#if tableRows.length > 0}
  <Table
    {columns}
    rows={tableRows}
    rowHeight={32}
    maxHeight="calc(100vh - 200px)"
  />
{:else}
  <EmptyState title={fr.audit_empty_title} description={fr.audit_empty_desc} />
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
</style>
