// Connections store — fed by real-time events, with filters.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { ConnectionEvent, DomainEventPayload } from '$lib/types';

// Connection map keyed by ID
export const connections = writable<Map<string, ConnectionEvent>>(new Map());

// Filters
export const connectionFilters = writable({
  search: '',
  protocol: '',
  verdict: '',
  direction: '',
});

// Sorted connection list
export const connectionList = derived(connections, ($conns) => {
  return Array.from($conns.values()).sort(
    (a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime()
  );
});

// Filtered connections
export const filteredConnections = derived(
  [connectionList, connectionFilters],
  ([$list, $filters]) => {
    return $list.filter((conn) => {
      // Search filter
      if ($filters.search) {
        const q = $filters.search.toLowerCase();
        const searchable = [
          conn.process_name,
          conn.source?.ip,
          conn.destination?.ip,
          conn.pid?.toString(),
          conn.user,
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase();
        if (!searchable.includes(q)) return false;
      }

      // Protocol filter
      if ($filters.protocol && conn.protocol.toLowerCase() !== $filters.protocol.toLowerCase()) {
        return false;
      }

      // Verdict filter
      if ($filters.verdict && conn.verdict !== $filters.verdict) {
        return false;
      }

      // Direction filter
      if ($filters.direction && conn.direction !== $filters.direction) {
        return false;
      }

      return true;
    });
  }
);

// Connection counts
export const connectionCounts = derived(connections, ($conns) => {
  let total = 0;
  let allowed = 0;
  let blocked = 0;
  let pending = 0;

  for (const conn of $conns.values()) {
    if (conn.state !== 'closed') {
      total++;
    }
    if (conn.verdict === 'allowed') allowed++;
    else if (conn.verdict === 'blocked') blocked++;
    else if (conn.verdict === 'pending_decision') pending++;
  }

  return { total, allowed, blocked, pending };
});

export function initConnectionListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://connection-detected', (event) => {
    try {
      const conn: ConnectionEvent = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        map.set(conn.id, conn);
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://connection-updated', (event) => {
    try {
      const update = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        const existing = map.get(update.id);
        if (existing) {
          map.set(update.id, { ...existing, state: update.state });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://connection-closed', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      connections.update((map) => {
        const existing = map.get(id);
        if (existing) {
          map.set(id, { ...existing, state: 'closed' });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-matched', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        const existing = map.get(payload.connection_id);
        if (existing) {
          map.set(payload.connection_id, {
            ...existing,
            verdict: payload.verdict,
            matched_rule: payload.rule_id,
          });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
