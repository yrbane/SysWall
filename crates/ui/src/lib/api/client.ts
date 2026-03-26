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

// --- Decisions ---

export async function listPendingDecisions(): Promise<PendingDecisionMessage[]> {
  return invoke<PendingDecisionMessage[]>('list_pending_decisions');
}

export async function respondToDecision(input: DecisionResponse): Promise<string> {
  return invoke<string>('respond_to_decision', { input });
}
