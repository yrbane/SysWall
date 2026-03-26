//! Rule commands — CRUD operations on firewall rules.

use tauri::State;

use syswall_proto::syswall::{
    CreateRuleRequest as ProtoCreateRule, RuleFiltersRequest, RuleIdRequest, ToggleRuleRequest,
};

use crate::grpc_client::GrpcState;

/// Serializable rule for the frontend.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct RuleResult {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
    pub criteria_json: String,
    pub effect: String,
    pub scope_json: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a rule from the frontend.
#[derive(serde::Deserialize)]
pub struct CreateRuleInput {
    pub name: String,
    pub priority: u32,
    pub criteria_json: String,
    pub effect: String,
    pub scope_json: String,
    pub source: String,
}

/// List all rules from the daemon.
#[tauri::command]
pub async fn list_rules(
    state: State<'_, GrpcState>,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<RuleResult>, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .list_rules(RuleFiltersRequest {
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(1000),
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rules = response
        .into_inner()
        .rules
        .into_iter()
        .map(|r| RuleResult {
            id: r.id,
            name: r.name,
            priority: r.priority,
            enabled: r.enabled,
            criteria_json: r.criteria_json,
            effect: r.effect,
            scope_json: r.scope_json,
            source: r.source,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(rules)
}

/// Create a new rule.
#[tauri::command]
pub async fn create_rule(
    state: State<'_, GrpcState>,
    input: CreateRuleInput,
) -> Result<RuleResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .create_rule(ProtoCreateRule {
            name: input.name,
            priority: input.priority,
            criteria_json: input.criteria_json,
            effect: input.effect,
            scope_json: input.scope_json,
            source: input.source,
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rule = response
        .into_inner()
        .rule
        .ok_or_else(|| "No rule in response".to_string())?;

    Ok(RuleResult {
        id: rule.id,
        name: rule.name,
        priority: rule.priority,
        enabled: rule.enabled,
        criteria_json: rule.criteria_json,
        effect: rule.effect,
        scope_json: rule.scope_json,
        source: rule.source,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    })
}

/// Delete a rule by ID.
#[tauri::command]
pub async fn delete_rule(state: State<'_, GrpcState>, id: String) -> Result<(), String> {
    let mut client = state.get_client().await?;

    client
        .control
        .delete_rule(RuleIdRequest { id })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    Ok(())
}

/// Toggle a rule enabled/disabled.
#[tauri::command]
pub async fn toggle_rule(
    state: State<'_, GrpcState>,
    id: String,
    enabled: bool,
) -> Result<RuleResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .toggle_rule(ToggleRuleRequest { id, enabled })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rule = response
        .into_inner()
        .rule
        .ok_or_else(|| "No rule in response".to_string())?;

    Ok(RuleResult {
        id: rule.id,
        name: rule.name,
        priority: rule.priority,
        enabled: rule.enabled,
        criteria_json: rule.criteria_json,
        effect: rule.effect,
        scope_json: rule.scope_json,
        source: rule.source,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    })
}
