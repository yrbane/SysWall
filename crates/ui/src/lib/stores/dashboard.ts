// Dashboard derived stats — aggregated from other stores.

import { derived, writable, get } from 'svelte/store';
import { connectionCounts, connections } from './connections';
import { firewallStatus } from './status';
import { auditEvents } from './audit';

// Traffic trend: ring buffer of data points (connections per second)
const TREND_BUFFER_SIZE = 60;
export const trafficTrend = writable<{ allowed: number; blocked: number }[]>(
  Array(TREND_BUFFER_SIZE).fill({ allowed: 0, blocked: 0 })
);

// Periodically sample connection counts for the trend chart
let trendInterval: ReturnType<typeof setInterval> | null = null;

export function startTrafficTrend(): void {
  if (trendInterval) return;

  let prevAllowed = 0;
  let prevBlocked = 0;

  trendInterval = setInterval(() => {
    const $c = get(connectionCounts);
    const newAllowed = $c.allowed - prevAllowed;
    const newBlocked = $c.blocked - prevBlocked;
    prevAllowed = $c.allowed;
    prevBlocked = $c.blocked;

    trafficTrend.update((buf) => {
      const updated = [...buf.slice(1), { allowed: Math.max(0, newAllowed), blocked: Math.max(0, newBlocked) }];
      return updated;
    });
  }, 1000);
}

export function stopTrafficTrend(): void {
  if (trendInterval) {
    clearInterval(trendInterval);
    trendInterval = null;
  }
}

// Top applications by connection count
export const topApps = derived(connections, ($conns) => {
  const counts = new Map<string, number>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const name = conn.process_name || 'Inconnu';
    counts.set(name, (counts.get(name) || 0) + 1);
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([name, count]) => ({ name, count }));
});

// Top destinations by IP
export const topDestinations = derived(connections, ($conns) => {
  const counts = new Map<string, number>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const ip = conn.destination?.ip || 'Inconnu';
    counts.set(ip, (counts.get(ip) || 0) + 1);
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([ip, count]) => ({ ip, count }));
});

// Recent alerts (system errors from audit)
export const recentAlerts = derived(auditEvents, ($events) => {
  return $events
    .filter((e) => e.severity === 'error' || e.severity === 'warning' || e.severity === 'critical')
    .slice(0, 5);
});

// Dashboard summary
export const dashboardSummary = derived(
  [connectionCounts, firewallStatus],
  ([$counts, $status]) => ({
    activeConnections: $counts.total,
    allowed: $counts.allowed,
    blocked: $counts.blocked,
    firewallEnabled: $status.enabled,
    version: $status.version,
    uptime: $status.uptime_secs,
    nftablesSynced: $status.nftables_synced,
  })
);
