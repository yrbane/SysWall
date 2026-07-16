// Typed API client — wrappers around Tauri invoke().

import { invoke } from '@tauri-apps/api/core';
import type {
  StatusResponse,
  RuleMessage,
  CreateRuleRequest,
  PendingDecisionMessage,
  DecisionResponse,
} from '$lib/types';

// --- Status ---

export async function getStatus(): Promise<StatusResponse> {
  return invoke<StatusResponse>('get_status');
}

// --- Rules ---

export async function listRules(offset = 0, limit = 1000): Promise<RuleMessage[]> {
  return invoke<RuleMessage[]>('list_rules', { offset, limit });
}

export async function createRule(input: CreateRuleRequest): Promise<RuleMessage> {
  return invoke<RuleMessage>('create_rule', { input });
}

export async function deleteRule(id: string): Promise<void> {
  return invoke<void>('delete_rule', { id });
}

export async function toggleRule(id: string, enabled: boolean): Promise<RuleMessage> {
  return invoke<RuleMessage>('toggle_rule', { id, enabled });
}

// --- Connections ---

// Un événement de connexion active (reflète DomainEventMessage côté daemon).
// An active-connection event (mirrors the daemon's DomainEventMessage).
export interface ActiveConnectionEvent {
  event_type: string;
  payload_json: string;
  timestamp: string;
}

// Instantané des connexions actives pour amorcer le store au montage.
// Snapshot of active connections used to seed the store on mount.
export async function getActiveConnections(): Promise<ActiveConnectionEvent[]> {
  return invoke<ActiveConnectionEvent[]>('get_active_connections');
}

// --- Decisions ---

export async function listPendingDecisions(): Promise<PendingDecisionMessage[]> {
  return invoke<PendingDecisionMessage[]>('list_pending_decisions');
}

export async function respondToDecision(input: DecisionResponse): Promise<string> {
  return invoke<string>('respond_to_decision', { input });
}

// --- Network ---

export async function setNetworkEnabled(enabled: boolean): Promise<void> {
  return invoke<void>('set_network_enabled', { enabled });
}

// --- Process ---

export interface ProcessDetails {
  pid: number;
  name: string;
  exe: string;
  cmdline: string;
  cwd: string;
  user: string;
  uid: number;
  state: string;
  threads: number;
  memory_rss_kb: number;
  open_fds: number;
  start_time: string;
  ports: PortInfo[];
  environ: string[];
}

export interface PortInfo {
  protocol: string;
  local_port: number;
  remote: string;
  state: string;
}

export async function getProcessDetails(pid: number): Promise<ProcessDetails> {
  return invoke<ProcessDetails>('get_process_details', { pid });
}

/// Lit un fichier icône système et retourne un data URI base64.
/// Read a system icon file and return a base64 data URI.
export async function readIcon(path: string): Promise<string> {
  return invoke<string>('read_icon', { path });
}
