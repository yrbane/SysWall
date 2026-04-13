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

// Mapping ports connus vers noms de service / Well-known port names
const WELL_KNOWN_PORTS: Record<number, string> = {
  22: 'SSH', 25: 'SMTP', 53: 'DNS', 80: 'HTTP', 110: 'POP3',
  143: 'IMAP', 443: 'HTTPS', 465: 'SMTPS', 587: 'SMTP', 853: 'DoT',
  993: 'IMAPS', 995: 'POP3S', 3306: 'MySQL', 5432: 'PostgreSQL',
  5900: 'VNC', 6379: 'Redis', 8080: 'HTTP-Alt', 8443: 'HTTPS-Alt',
  9090: 'Prometheus', 27017: 'MongoDB',
};

function portLabel(port: number): string {
  return WELL_KNOWN_PORTS[port] || String(port);
}

// Top applications by connection count (avec icône)
// Top applications by connection count (with icon)
export const topApps = derived(connections, ($conns) => {
  const apps = new Map<string, { count: number; icon?: string }>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const name = conn.process_name || 'Inconnu';
    const existing = apps.get(name);
    if (existing) {
      existing.count++;
      if (!existing.icon && conn.icon) existing.icon = conn.icon;
    } else {
      apps.set(name, { count: 1, icon: conn.icon });
    }
  }
  return Array.from(apps.entries())
    .sort((a, b) => b[1].count - a[1].count)
    .slice(0, 5)
    .map(([name, data]) => ({ name, count: data.count, icon: data.icon }));
});

// Top destinations par IP:port (avec nom de service)
// Top destinations by IP:port (with service name)
export const topDestinations = derived(connections, ($conns) => {
  const dests = new Map<string, { ip: string; port: number; count: number }>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const ip = conn.destination?.ip || 'Inconnu';
    const port = conn.destination?.port || 0;
    const key = `${ip}:${port}`;
    const existing = dests.get(key);
    if (existing) {
      existing.count++;
    } else {
      dests.set(key, { ip, port, count: 1 });
    }
  }
  return Array.from(dests.values())
    .sort((a, b) => b.count - a.count)
    .slice(0, 5)
    .map((d) => ({ ip: d.ip, port: d.port, service: portLabel(d.port), count: d.count }));
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
