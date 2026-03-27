<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    filteredConnections,
    connectionFilters,
    connectionCounts,
    connectionList,
  } from '$lib/stores/connections';
  import type { ConnectionEvent } from '$lib/types';

  // Sort state
  let sortKey = $state<string>('started_at');
  let sortDir = $state<'asc' | 'desc'>('desc');

  // Expanded row
  let expandedId = $state<string | null>(null);

  // Filters bound to the store
  let searchValue = $state('');
  let protocolFilter = $state('');
  let verdictFilter = $state('');
  let directionFilter = $state('');
  let applicationFilter = $state('');
  let portFilter = $state('');

  // Unique application names derived from all connections
  const uniqueApps = $derived.by(() => {
    const apps = new Set<string>();
    for (const conn of $connectionList) {
      if (conn.process_name) apps.add(conn.process_name);
    }
    return [...apps].sort((a, b) => a.localeCompare(b, 'fr'));
  });

  // Sync local state to store
  $effect(() => {
    connectionFilters.set({
      search: searchValue,
      protocol: protocolFilter,
      verdict: verdictFilter,
      direction: directionFilter,
    });
  });

  // Apply additional local filters (app, port) on top of store-filtered results
  const localFiltered = $derived.by(() => {
    let list = $filteredConnections;

    // Application filter
    if (applicationFilter) {
      list = list.filter((c) => c.process_name === applicationFilter);
    }

    // Port filter (match source or destination port)
    if (portFilter) {
      const portNum = parseInt(portFilter, 10);
      if (!isNaN(portNum)) {
        list = list.filter(
          (c) => c.source?.port === portNum || c.destination?.port === portNum
        );
      }
    }

    return list;
  });

  // Sort the filtered connections
  const sortedConnections = $derived.by(() => {
    const list = [...localFiltered];
    list.sort((a, b) => {
      let valA: string | number = '';
      let valB: string | number = '';

      switch (sortKey) {
        case 'process_name':
          valA = a.process_name || '';
          valB = b.process_name || '';
          break;
        case 'pid':
          valA = a.pid || 0;
          valB = b.pid || 0;
          break;
        case 'user':
          valA = a.user || '';
          valB = b.user || '';
          break;
        case 'source_ip':
          valA = a.source?.ip || '';
          valB = b.source?.ip || '';
          break;
        case 'source_port':
          valA = a.source?.port || 0;
          valB = b.source?.port || 0;
          break;
        case 'dest_ip':
          valA = a.destination?.ip || '';
          valB = b.destination?.ip || '';
          break;
        case 'dest_port':
          valA = a.destination?.port || 0;
          valB = b.destination?.port || 0;
          break;
        case 'protocol':
          valA = a.protocol;
          valB = b.protocol;
          break;
        case 'state':
          valA = a.state;
          valB = b.state;
          break;
        case 'verdict':
          valA = a.verdict;
          valB = b.verdict;
          break;
        default:
          valA = new Date(a.started_at).getTime();
          valB = new Date(b.started_at).getTime();
      }

      if (typeof valA === 'number' && typeof valB === 'number') {
        return sortDir === 'asc' ? valA - valB : valB - valA;
      }
      const cmp = String(valA).localeCompare(String(valB));
      return sortDir === 'asc' ? cmp : -cmp;
    });
    return list;
  });

  function toggleSort(key: string) {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  function verdictVariant(verdict: string): 'green' | 'red' | 'orange' | 'neutral' {
    if (verdict === 'allowed') return 'green';
    if (verdict === 'blocked') return 'red';
    if (verdict === 'pending_decision') return 'orange';
    return 'neutral';
  }

  function verdictLabel(verdict: string): string {
    if (verdict === 'allowed') return fr.conn_allowed;
    if (verdict === 'blocked') return fr.conn_blocked;
    if (verdict === 'pending_decision') return fr.conn_pending;
    return fr.conn_unknown;
  }

  function clearFilters() {
    searchValue = '';
    protocolFilter = '';
    verdictFilter = '';
    directionFilter = '';
    applicationFilter = '';
    portFilter = '';
  }

  function formatAddr(addr: { ip: string; port: number } | undefined): string {
    if (!addr) return '--';
    return `${addr.ip}:${addr.port}`;
  }

  // Resolve an app icon path to a displayable src URL
  function resolveIconSrc(iconPath: string | undefined): string | null {
    if (!iconPath) return null;
    try {
      return convertFileSrc(iconPath);
    } catch {
      return null;
    }
  }

  // Track broken icon images to fall back to generic icon
  let brokenIcons = $state(new Set<string>());

  function handleIconError(connId: string) {
    brokenIcons = new Set([...brokenIcons, connId]);
  }

  const hasActiveFilters = $derived(
    searchValue || protocolFilter || verdictFilter || directionFilter || applicationFilter || portFilter
  );

  // Column definitions for sort headers
  const columns = [
    { key: 'process_name', label: fr.conn_application },
    { key: 'pid', label: fr.conn_pid },
    { key: 'user', label: fr.conn_user },
    { key: 'source_ip', label: fr.conn_source_ip },
    { key: 'source_port', label: fr.conn_source_port },
    { key: 'dest_ip', label: fr.conn_dest_ip },
    { key: 'dest_port', label: fr.conn_dest_port },
    { key: 'protocol', label: fr.conn_protocol },
    { key: 'state', label: fr.conn_state },
    { key: 'verdict', label: fr.conn_verdict },
    { key: 'matched_rule', label: fr.conn_rule },
  ] as const;
</script>

<div class="page-header">
  <div class="page-header-left">
    <h1 class="page-title">{fr.nav_connections}</h1>
    <Badge variant="cyan" label="{$connectionCounts.total} {fr.conn_count}" />
  </div>
  <div class="live-indicator">
    <span class="live-dot"></span>
    <span class="text-xs text-secondary">{fr.conn_live}</span>
  </div>
</div>

<!-- Filter bar -->
<div class="filter-bar">
  <div class="filter-search">
    <Input
      type="search"
      placeholder={fr.conn_search}
      bind:value={searchValue}
    />
  </div>

  <select class="filter-select" bind:value={protocolFilter}>
    <option value="">{fr.conn_filter_all} - {fr.conn_filter_protocol}</option>
    <option value="tcp">TCP</option>
    <option value="udp">UDP</option>
    <option value="icmp">ICMP</option>
  </select>

  <select class="filter-select" bind:value={verdictFilter}>
    <option value="">{fr.conn_filter_all} - {fr.conn_filter_verdict}</option>
    <option value="allowed">{fr.conn_allowed}</option>
    <option value="blocked">{fr.conn_blocked}</option>
    <option value="pending_decision">{fr.conn_pending}</option>
  </select>

  <select class="filter-select" bind:value={directionFilter}>
    <option value="">{fr.conn_filter_all} - {fr.conn_filter_direction}</option>
    <option value="inbound">{fr.conn_inbound}</option>
    <option value="outbound">{fr.conn_outbound}</option>
  </select>

  <select class="filter-select" bind:value={applicationFilter}>
    <option value="">{fr.conn_all_apps}</option>
    {#each uniqueApps as appName}
      <option value={appName}>{appName}</option>
    {/each}
  </select>

  <div class="filter-port">
    <Input
      type="text"
      placeholder={fr.conn_filter_port}
      bind:value={portFilter}
    />
  </div>

  {#if hasActiveFilters}
    <Button variant="ghost" size="sm" onclick={clearFilters}>
      {fr.conn_clear_filters}
    </Button>
  {/if}
</div>

<!-- Connections table -->
{#if sortedConnections.length > 0}
  <div class="table-wrapper">
    <div class="table-header-row">
      {#each columns as col}
        <button
          class="th-cell"
          class:sorted={sortKey === col.key}
          onclick={() => toggleSort(col.key)}
        >
          {col.label}
          {#if sortKey === col.key}
            <span class="sort-arrow">{sortDir === 'asc' ? '▲' : '▼'}</span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="table-body">
      {#each sortedConnections as conn (conn.id)}
        <div
          class="table-row"
          class:expanded={expandedId === conn.id}
          onclick={() => toggleExpand(conn.id)}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === 'Enter' && toggleExpand(conn.id)}
        >
          <div class="td-cell truncate app-cell">
            {#if conn.icon && resolveIconSrc(conn.icon) && !brokenIcons.has(conn.id)}
              <img
                class="app-icon-img"
                src={resolveIconSrc(conn.icon)}
                alt=""
                width="16"
                height="16"
                onerror={() => handleIconError(conn.id)}
              />
            {:else}
              <svg class="app-icon-fallback" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-tertiary)" stroke-width="1.5">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>
            {/if}
            <span class="truncate">{conn.process_name || fr.conn_unknown}</span>
          </div>
          <div class="td-cell font-mono">{conn.pid || '--'}</div>
          <div class="td-cell truncate">{conn.user || '--'}</div>
          <div class="td-cell font-mono truncate">{conn.source?.ip || '--'}</div>
          <div class="td-cell font-mono">{conn.source?.port || '--'}</div>
          <div class="td-cell font-mono truncate">{conn.destination?.ip || '--'}</div>
          <div class="td-cell font-mono">{conn.destination?.port || '--'}</div>
          <div class="td-cell">
            <Badge variant="cyan" label={conn.protocol.toUpperCase()} />
          </div>
          <div class="td-cell">{conn.state}</div>
          <div class="td-cell">
            <Badge variant={verdictVariant(conn.verdict)} label={verdictLabel(conn.verdict)} />
          </div>
          <div class="td-cell font-mono truncate text-secondary">{conn.matched_rule || '--'}</div>
        </div>

        <!-- Expanded detail panel -->
        {#if expandedId === conn.id}
          <div class="detail-panel">
            <Card padding="sm">
              <div class="detail-grid">
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_connection_id}</span>
                  <span class="detail-value font-mono">{conn.id}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_application}</span>
                  <span class="detail-value">{conn.process_name || fr.conn_unknown}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_pid}</span>
                  <span class="detail-value font-mono">{conn.pid || '--'}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_user}</span>
                  <span class="detail-value">{conn.user || '--'}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_source}</span>
                  <span class="detail-value font-mono">{formatAddr(conn.source)}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_destination}</span>
                  <span class="detail-value font-mono">{formatAddr(conn.destination)}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_direction}</span>
                  <span class="detail-value">
                    {conn.direction === 'inbound' ? fr.conn_inbound : fr.conn_outbound}
                  </span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_protocol}</span>
                  <span class="detail-value font-mono">{conn.protocol.toUpperCase()}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_bytes_sent}</span>
                  <span class="detail-value font-mono">{conn.bytes_sent.toLocaleString('fr-FR')}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_bytes_received}</span>
                  <span class="detail-value font-mono">{conn.bytes_received.toLocaleString('fr-FR')}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_started_at}</span>
                  <span class="detail-value font-mono">
                    {new Date(conn.started_at).toLocaleString('fr-FR')}
                  </span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">{fr.conn_rule}</span>
                  <span class="detail-value font-mono">{conn.matched_rule || '--'}</span>
                </div>
              </div>
            </Card>
          </div>
        {/if}
      {/each}
    </div>
  </div>
{:else}
  <EmptyState message={fr.conn_no_connections} />
{/if}

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .page-header-left {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .live-indicator {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .live-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-green);
    box-shadow: var(--glow-green);
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  /* Filter bar */
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

  .filter-port {
    min-width: 130px;
    max-width: 160px;
  }

  /* Table */
  .table-wrapper {
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .table-header-row {
    display: flex;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
  }

  .th-cell {
    flex: 1;
    padding: var(--space-2) var(--space-2);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border: none;
    background: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    text-align: left;
    transition: color var(--transition-fast);
    white-space: nowrap;
  }

  .th-cell:hover {
    color: var(--text-primary);
  }

  .th-cell.sorted {
    color: var(--accent-cyan);
  }

  .sort-arrow {
    font-size: 0.6em;
  }

  .table-body {
    max-height: calc(100vh - 320px);
    overflow-y: auto;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: 0;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background var(--transition-fast);
    min-height: 40px;
  }

  .table-row:hover {
    background: var(--bg-hover);
  }

  .table-row.expanded {
    background: var(--bg-hover);
    border-bottom-color: var(--accent-cyan);
  }

  .td-cell {
    flex: 1;
    padding: var(--space-2);
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Detail panel */
  .detail-panel {
    padding: var(--space-3) var(--space-4);
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    animation: slideDown 200ms ease;
  }

  @keyframes slideDown {
    from { opacity: 0; max-height: 0; }
    to { opacity: 1; max-height: 500px; }
  }

  .detail-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3);
  }

  .detail-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .detail-label {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    word-break: break-all;
  }

  /* App icon in table cell */
  .app-cell {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .app-icon-img {
    flex-shrink: 0;
    border-radius: 2px;
    object-fit: contain;
  }

  .app-icon-fallback {
    flex-shrink: 0;
  }
</style>
