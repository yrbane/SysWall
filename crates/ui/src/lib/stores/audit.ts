// Audit events store — populated from domain events.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { AuditEvent, DomainEventPayload } from '$lib/types';

// Maximum events to keep in memory
const MAX_AUDIT_EVENTS = 5000;

export const auditEvents = writable<AuditEvent[]>([]);

// Filters
export const auditFilters = writable({
  search: '',
  severity: '',
  category: '',
  dateStart: '',
  dateEnd: '',
});

// Pagination
export const auditPage = writable(0);
export const auditPageSize = 50;

export const filteredAuditEvents = derived(
  [auditEvents, auditFilters],
  ([$events, $filters]) => {
    return $events.filter((evt) => {
      if ($filters.search) {
        const q = $filters.search.toLowerCase();
        if (!evt.description.toLowerCase().includes(q)) return false;
      }
      if ($filters.severity && evt.severity !== $filters.severity) return false;
      if ($filters.category && evt.category !== $filters.category) return false;
      if ($filters.dateStart && evt.timestamp < $filters.dateStart) return false;
      if ($filters.dateEnd && evt.timestamp > $filters.dateEnd) return false;
      return true;
    });
  }
);

export const totalFilteredCount = derived(filteredAuditEvents, ($e) => $e.length);
export const totalPages = derived(totalFilteredCount, ($c) =>
  Math.max(1, Math.ceil($c / auditPageSize))
);

export const paginatedAuditEvents = derived(
  [filteredAuditEvents, auditPage],
  ([$events, $page]) => {
    const start = $page * auditPageSize;
    return $events.slice(start, start + auditPageSize);
  }
);

// Build a human-readable description from a domain event payload
function buildDescription(eventType: string, payload: any): string {
  // Use explicit message/description if available
  if (payload.message && typeof payload.message === 'string') return payload.message;
  if (payload.description && typeof payload.description === 'string') return payload.description;

  const app = payload.process?.name || payload.process_name || '';
  const dstIp = payload.destination?.ip || '';
  const dstPort = payload.destination?.port || '';
  const dst = dstIp ? `${dstIp}${dstPort ? ':' + dstPort : ''}` : '';
  const proto = typeof payload.protocol === 'string' ? payload.protocol.toUpperCase() : '';
  const ruleId = payload.rule_id || payload.matched_rule || '';
  const verdict = payload.verdict || '';
  const connId = payload.connection_id || payload.id || '';

  switch (eventType) {
    case 'connection_detected':
      return `Connexion ${proto || ''} ${app ? 'de ' + app + ' ' : ''}vers ${dst || 'inconnu'}`.trim();
    case 'connection_updated':
      return `Connexion mise à jour${connId ? ' (' + connId.slice(0, 8) + ')' : ''}${payload.state ? ' — ' + payload.state : ''}`;
    case 'connection_closed':
      return `Connexion fermée${connId ? ' (' + (typeof connId === 'string' ? connId.slice(0, 8) : '') + ')' : ''}`;
    case 'rule_created':
      return `Règle créée : ${payload.name || ruleId || 'sans nom'}`;
    case 'rule_updated':
      return `Règle modifiée : ${payload.name || ruleId || 'sans nom'}`;
    case 'rule_deleted':
      return `Règle supprimée : ${payload.name || ruleId || 'sans nom'}`;
    case 'rule_matched':
      return `Règle ${ruleId ? ruleId.slice(0, 8) : '?'} appliquée${verdict ? ' — ' + verdict : ''}${connId ? ' (conn ' + connId.slice(0, 8) + ')' : ''}`;
    case 'decision_required':
      return `Décision requise${app ? ' pour ' + app : ''}${dst ? ' vers ' + dst : ''}`;
    case 'decision_resolved':
      return `Décision prise${payload.action ? ' : ' + payload.action : ''}${app ? ' pour ' + app : ''}`;
    case 'decision_expired':
      return `Décision expirée${connId ? ' (' + (typeof connId === 'string' ? connId.slice(0, 8) : '') + ')' : ''}`;
    case 'firewall_status_changed':
      return `État du pare-feu modifié${payload.enabled !== undefined ? ' — ' + (payload.enabled ? 'activé' : 'désactivé') : ''}`;
    case 'system_error':
      return `Erreur système${payload.error ? ' : ' + payload.error : ''}`;
    default:
      return eventType.replace(/_/g, ' ');
  }
}

// Extract clean metadata: flatten objects, skip nested objects/arrays, keep only simple key=value pairs
function cleanMetadata(payload: any): Record<string, string> {
  if (typeof payload !== 'object' || payload === null) return {};
  const result: Record<string, string> = {};
  for (const [key, val] of Object.entries(payload)) {
    if (val === null || val === undefined) continue;
    if (typeof val === 'object') {
      // Flatten one level of nested simple values
      if (!Array.isArray(val)) {
        for (const [subKey, subVal] of Object.entries(val as Record<string, unknown>)) {
          if (subVal !== null && subVal !== undefined && typeof subVal !== 'object') {
            result[`${key}.${subKey}`] = String(subVal);
          }
        }
      }
      continue;
    }
    result[key] = String(val);
  }
  return result;
}

function eventToAudit(eventType: string, payloadJson: string, timestamp: string): AuditEvent | null {
  try {
    const payload = JSON.parse(payloadJson);
    const categoryMap: Record<string, string> = {
      connection_detected: 'connection',
      connection_updated: 'connection',
      connection_closed: 'connection',
      rule_created: 'rule',
      rule_updated: 'rule',
      rule_deleted: 'rule',
      rule_matched: 'rule',
      decision_required: 'decision',
      decision_resolved: 'decision',
      decision_expired: 'decision',
      firewall_status_changed: 'system',
      system_error: 'system',
    };

    const severityMap: Record<string, string> = {
      connection_detected: 'info',
      connection_updated: 'debug',
      connection_closed: 'info',
      rule_created: 'info',
      rule_updated: 'info',
      rule_deleted: 'warning',
      rule_matched: 'debug',
      decision_required: 'warning',
      decision_resolved: 'info',
      decision_expired: 'warning',
      firewall_status_changed: 'info',
      system_error: payload.severity || 'error',
    };

    const description = buildDescription(eventType, payload);

    return {
      id: crypto.randomUUID(),
      timestamp,
      severity: severityMap[eventType] || 'info',
      category: categoryMap[eventType] || 'system',
      description,
      metadata: cleanMetadata(payload),
    };
  } catch {
    return null;
  }
}

export function initAuditListener(): () => void {
  const eventTypes = [
    'syswall://connection-detected',
    'syswall://connection-closed',
    'syswall://rule-created',
    'syswall://rule-updated',
    'syswall://rule-deleted',
    'syswall://decision-required',
    'syswall://decision-resolved',
    'syswall://decision-expired',
    'syswall://status-changed',
    'syswall://system-error',
  ];

  const unlisteners: (() => void)[] = [];

  for (const eventName of eventTypes) {
    listen<DomainEventPayload>(eventName, (event) => {
      const audit = eventToAudit(
        event.payload.event_type,
        event.payload.payload_json,
        event.payload.timestamp
      );
      if (audit) {
        auditEvents.update((list) => {
          const updated = [audit, ...list];
          if (updated.length > MAX_AUDIT_EVENTS) {
            return updated.slice(0, MAX_AUDIT_EVENTS);
          }
          return updated;
        });
      }
    }).then((fn) => unlisteners.push(fn));
  }

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
