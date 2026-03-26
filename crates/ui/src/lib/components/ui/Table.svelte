<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  interface Column {
    key: string;
    label: string;
    mono?: boolean;
    width?: string;
  }

  interface Props {
    columns: Column[];
    rows: T[];
    rowHeight?: number;
    maxHeight?: string;
    onrowclick?: (row: T) => void;
    renderCell?: Snippet<[{ row: T; column: Column }]>;
  }

  let {
    columns,
    rows,
    rowHeight = 40,
    maxHeight = '100%',
    onrowclick,
    renderCell,
  }: Props = $props();

  // Virtual scroll state
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  const BUFFER = 5;
  const totalHeight = $derived(rows.length * rowHeight);
  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - BUFFER));
  const endIndex = $derived(
    Math.min(rows.length, Math.ceil((scrollTop + containerHeight) / rowHeight) + BUFFER)
  );
  const visibleRows = $derived(rows.slice(startIndex, endIndex));
  const offsetY = $derived(startIndex * rowHeight);

  function handleScroll(e: Event) {
    const target = e.target as HTMLDivElement;
    scrollTop = target.scrollTop;
    containerHeight = target.clientHeight;
  }

  function getCellValue(row: T, key: string): string {
    const val = (row as Record<string, unknown>)[key];
    if (val === null || val === undefined) return '--';
    return String(val);
  }
</script>

<div class="table-wrapper" style="max-height: {maxHeight};">
  <div class="table-header">
    <div class="table-row header-row">
      {#each columns as col}
        <div class="table-cell header-cell" style={col.width ? `width: ${col.width}` : ''}>
          {col.label}
        </div>
      {/each}
    </div>
  </div>
  <div
    class="table-body"
    bind:this={scrollContainer}
    onscroll={handleScroll}
  >
    <div class="virtual-spacer" style="height: {totalHeight}px;">
      <div class="virtual-content" style="transform: translateY({offsetY}px);">
        {#each visibleRows as row, i (startIndex + i)}
          <div
            class="table-row body-row"
            style="height: {rowHeight}px;"
            onclick={() => onrowclick?.(row)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && onrowclick?.(row)}
          >
            {#each columns as col}
              <div
                class="table-cell"
                class:font-mono={col.mono}
                style={col.width ? `width: ${col.width}` : ''}
              >
                {#if renderCell}
                  {@render renderCell({ row, column: col })}
                {:else}
                  {getCellValue(row, col.key)}
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .table-wrapper {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .table-header {
    flex-shrink: 0;
  }

  .table-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .virtual-spacer {
    position: relative;
  }

  .virtual-content {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: 0 var(--space-4);
  }

  .header-row {
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    height: 36px;
  }

  .body-row {
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .body-row:hover {
    background: var(--bg-hover);
  }

  .table-cell {
    flex: 1;
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 var(--space-2);
  }

  .header-cell {
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
</style>
