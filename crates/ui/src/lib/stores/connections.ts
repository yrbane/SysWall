// Connections store — fed by real-time events, with filters.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { ConnectionEvent, DomainEventPayload } from '$lib/types';

// Flatten a raw Rust-serialized Connection into our flat ConnectionEvent format
function flattenConnection(raw: any): ConnectionEvent {
  return {
    id: raw.id?.['0'] || raw.id || '',
    protocol: typeof raw.protocol === 'string' ? raw.protocol : raw.protocol?.Tcp ? 'Tcp' : raw.protocol?.Udp ? 'Udp' : 'Other',
    source: {
      ip: raw.source?.ip || '',
      port: typeof raw.source?.port === 'number' ? raw.source.port : raw.source?.port?.['0'] ?? raw.source?.port ?? 0,
    },
    destination: {
      ip: raw.destination?.ip || '',
      port: typeof raw.destination?.port === 'number' ? raw.destination.port : raw.destination?.port?.['0'] ?? raw.destination?.port ?? 0,
    },
    direction: typeof raw.direction === 'string' ? raw.direction : Object.keys(raw.direction || {})[0] || 'Outbound',
    state: typeof raw.state === 'string' ? raw.state : Object.keys(raw.state || {})[0] || 'New',
    process_name: raw.process?.name || raw.process_name || undefined,
    process_path: raw.process?.path?.['0'] || raw.process?.path || raw.process_path || undefined,
    pid: raw.process?.pid || raw.pid || undefined,
    user: raw.user?.name || raw.user || undefined,
    icon: raw.process?.icon || undefined,
    bytes_sent: raw.bytes_sent || 0,
    bytes_received: raw.bytes_received || 0,
    started_at: raw.started_at || new Date().toISOString(),
    verdict: typeof raw.verdict === 'string' ? raw.verdict : Object.keys(raw.verdict || {})[0] || 'Unknown',
    matched_rule: raw.matched_rule?.['0'] || raw.matched_rule || undefined,
  };
}

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
      const raw = JSON.parse(event.payload.payload_json);
      const conn = flattenConnection(raw);
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
