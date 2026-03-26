// Firewall status store — fetched on demand, updated by events.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { getStatus } from '$lib/api/client';
import type { StatusResponse, DomainEventPayload } from '$lib/types';

const defaultStatus: StatusResponse = {
  enabled: false,
  active_rules_count: 0,
  nftables_synced: false,
  uptime_secs: 0,
  version: '',
};

export const firewallStatus = writable<StatusResponse>(defaultStatus);
export const statusError = writable<string | null>(null);
export const statusLoading = writable(true);

export const isFirewallActive = derived(firewallStatus, ($s) => $s.enabled);

export async function fetchStatus(): Promise<void> {
  statusLoading.set(true);
  statusError.set(null);
  try {
    const status = await getStatus();
    firewallStatus.set(status);
  } catch (e) {
    statusError.set(String(e));
  } finally {
    statusLoading.set(false);
  }
}

export function initStatusListener(): () => void {
  let unlisten: (() => void) | undefined;

  listen<DomainEventPayload>('syswall://status-changed', (event) => {
    try {
      const status: StatusResponse = JSON.parse(event.payload.payload_json);
      firewallStatus.set(status);
    } catch {
      // Ignore parse errors
    }
  }).then((fn) => {
    unlisten = fn;
  });

  return () => {
    unlisten?.();
  };
}
