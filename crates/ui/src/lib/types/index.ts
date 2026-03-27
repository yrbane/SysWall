// TypeScript types mirroring proto messages and domain types.

export interface StatusResponse {
  enabled: boolean;
  active_rules_count: number;
  nftables_synced: boolean;
  uptime_secs: number;
  version: string;
}

export interface RuleMessage {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
  criteria_json: string;
  effect: string;
  scope_json: string;
  source: string;
  created_at: string;
  updated_at: string;
}

export interface RuleCriteria {
  application?: { name?: string; path?: string };
  user?: string;
  remote_ip?: { exact?: string; cidr?: string };
  remote_port?: { exact?: number; range?: [number, number] };
  local_port?: { exact?: number; range?: [number, number] };
  protocol?: string;
  direction?: string;
}

export interface RuleScope {
  type: 'permanent' | 'temporary';
  expires_at?: string;
}

export interface CreateRuleRequest {
  name: string;
  priority: number;
  criteria_json: string;
  effect: string;
  scope_json: string;
  source: string;
}

export interface PendingDecisionMessage {
  id: string;
  snapshot_json: string;
  requested_at: string;
  expires_at: string;
  status: string;
}

export interface ConnectionSnapshot {
  protocol: string;
  source: { ip: string; port: number };
  destination: { ip: string; port: number };
  direction: string;
  process_name?: string;
  process_path?: string;
  user?: string;
  icon?: string;
}

export interface DecisionResponse {
  pending_decision_id: string;
  action: string;
  granularity: string;
}

export interface DomainEventPayload {
  event_type: string;
  payload_json: string;
  timestamp: string;
}

export interface ConnectionEvent {
  id: string;
  protocol: string;
  source: { ip: string; port: number };
  destination: { ip: string; port: number };
  direction: string;
  state: string;
  process_name?: string;
  process_path?: string;
  pid?: number;
  user?: string;
  icon?: string;
  bytes_sent: number;
  bytes_received: number;
  started_at: string;
  verdict: string;
  matched_rule?: string;
}

export interface AuditEvent {
  id: string;
  timestamp: string;
  severity: string;
  category: string;
  description: string;
  metadata: Record<string, string>;
}

export type Verdict = 'allowed' | 'blocked' | 'pending_decision' | 'unknown' | 'ignored';
export type Protocol = 'tcp' | 'udp' | 'icmp' | 'other';
export type Direction = 'inbound' | 'outbound';
export type Severity = 'debug' | 'info' | 'warning' | 'error' | 'critical';
export type EventCategory = 'connection' | 'rule' | 'decision' | 'system' | 'config';
export type RuleEffect = 'allow' | 'block' | 'ask' | 'observe';
export type RuleSource = 'manual' | 'auto_learning' | 'import' | 'system';
export type DecisionAction = 'allow_once' | 'block_once' | 'always_allow' | 'always_block' | 'create_rule' | 'ignore';
export type DecisionGranularity = 'app_only' | 'app_and_destination' | 'app_and_protocol' | 'full';
