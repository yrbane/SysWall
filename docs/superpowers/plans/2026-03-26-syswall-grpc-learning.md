# SysWall gRPC Services + Auto-Learning Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the gRPC control and event services so the UI can connect to the daemon over Unix socket, manage rules, view pending decisions, respond to auto-learning prompts, and receive real-time domain events. Wire the periodic decision expiration task into the supervisor.

**Architecture:** The daemon crate gets four new files in `grpc/`: control_service.rs (7 RPCs delegating to app services), event_service.rs (server-side streaming from EventBus), converters.rs (proto <-> domain type conversions), and server.rs (tonic on Unix socket). The supervisor spawns the gRPC server and a periodic expiration task.

**Tech Stack:** Rust, tonic (gRPC server), prost (proto types), tokio (Unix socket, broadcast stream), serde_json (JSON serialization for proto fields), nix (socket permissions), hyper/tower (tonic transport layer)

**Spec:** `docs/superpowers/specs/2026-03-26-syswall-grpc-learning-design.md`

---

## File Map

### crates/daemon/src/grpc/
| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports all gRPC modules (replace empty stub) |
| `converters.rs` | Pure functions: Rule <-> RuleMessage, PendingDecision <-> PendingDecisionMessage, CreateRuleRequest -> CreateRuleCommand, DecisionResponseRequest -> RespondToDecisionCommand, DomainEvent -> DomainEventMessage, error mapping |
| `control_service.rs` | SysWallControlService implementing the generated SysWallControl trait (7 RPCs) |
| `event_service.rs` | SysWallEventService implementing the generated SysWallEvents trait (SubscribeEvents streaming) |
| `server.rs` | start_grpc_server(): Unix socket listener, socket permissions, tonic router, cancellation |

### crates/daemon/src/ (modified)
| File | Changes |
|---|---|
| `main.rs` | Add grpc-server supervisor task, add decision-expiry supervisor task |

### Root (modified)
| File | Changes |
|---|---|
| `Cargo.toml` | Add hyper, hyper-util, tower, http workspace dependencies |
| `crates/daemon/Cargo.toml` | Add serde_json, uuid, nix, hyper, hyper-util, tower, http dependencies |

---

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/daemon/Cargo.toml`

- [ ] **Step 1: Add workspace dependencies for gRPC Unix socket transport**

`Cargo.toml` -- add to `[workspace.dependencies]`:
```toml
hyper = { version = "1", features = ["server"] }
hyper-util = { version = "0.1", features = ["tokio"] }
tower = { version = "0.5" }
http = "1"
http-body = "1"
```

- [ ] **Step 2: Add daemon crate dependencies**

`crates/daemon/Cargo.toml` -- add to `[dependencies]`:
```toml
serde_json = { workspace = true }
uuid = { workspace = true }
nix = { workspace = true }
hyper = { workspace = true }
hyper-util = { workspace = true }
tower = { workspace = true }
http = { workspace = true }
http-body = { workspace = true }
```

- [ ] **Step 3: Verify workspace compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 2: Proto <-> Domain Converters

**Files:**
- Create: `crates/daemon/src/grpc/converters.rs`
- Modify: `crates/daemon/src/grpc/mod.rs`

- [ ] **Step 1: Create converters.rs with error mapping helper**

`crates/daemon/src/grpc/converters.rs`:
```rust
use syswall_domain::entities::{
    DecisionAction, DecisionGranularity, PendingDecision, PendingDecisionStatus, Rule, RuleCriteria,
    RuleEffect, RuleScope, RuleSource,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_proto::syswall::{
    CreateRuleRequest, DecisionResponseRequest, DomainEventMessage, PendingDecisionMessage,
    RuleMessage,
};

use chrono::Utc;
use syswall_app::commands::{CreateRuleCommand, RespondToDecisionCommand};
use syswall_domain::entities::PendingDecisionId;
use uuid::Uuid;

// --- Error Mapping ---

pub fn domain_error_to_status(e: DomainError) -> tonic::Status {
    match e {
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::AlreadyExists(msg) => tonic::Status::already_exists(msg),
        DomainError::Infrastructure(msg) => tonic::Status::internal(msg),
        DomainError::NotPermitted(msg) => tonic::Status::permission_denied(msg),
    }
}

// --- Rule Conversions ---

pub fn rule_to_proto(rule: &Rule) -> RuleMessage {
    RuleMessage {
        id: rule.id.as_uuid().to_string(),
        name: rule.name.clone(),
        priority: rule.priority.value(),
        enabled: rule.enabled,
        criteria_json: serde_json::to_string(&rule.criteria).unwrap_or_default(),
        effect: rule_effect_to_string(rule.effect),
        scope_json: serde_json::to_string(&rule.scope).unwrap_or_default(),
        source: rule_source_to_string(rule.source),
        created_at: rule.created_at.to_rfc3339(),
        updated_at: rule.updated_at.to_rfc3339(),
    }
}

pub fn proto_to_create_rule_cmd(req: &CreateRuleRequest) -> Result<CreateRuleCommand, tonic::Status> {
    if req.name.trim().is_empty() {
        return Err(tonic::Status::invalid_argument("Rule name cannot be empty"));
    }

    let criteria: RuleCriteria = serde_json::from_str(&req.criteria_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid criteria_json: {}", e)))?;

    let effect = parse_rule_effect(&req.effect)?;

    let scope: RuleScope = serde_json::from_str(&req.scope_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid scope_json: {}", e)))?;

    let source = parse_rule_source(&req.source)?;

    Ok(CreateRuleCommand {
        name: req.name.clone(),
        priority: req.priority,
        criteria,
        effect,
        scope,
        source,
    })
}

// --- PendingDecision Conversions ---

pub fn pending_decision_to_proto(pd: &PendingDecision) -> PendingDecisionMessage {
    PendingDecisionMessage {
        id: pd.id.as_uuid().to_string(),
        snapshot_json: serde_json::to_string(&pd.connection_snapshot).unwrap_or_default(),
        requested_at: pd.requested_at.to_rfc3339(),
        expires_at: pd.expires_at.to_rfc3339(),
        status: pending_status_to_string(pd.status),
    }
}

pub fn proto_to_respond_cmd(
    req: &DecisionResponseRequest,
) -> Result<RespondToDecisionCommand, tonic::Status> {
    let uuid = Uuid::parse_str(&req.pending_decision_id)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid pending_decision_id: {}", e)))?;

    let action = parse_decision_action(&req.action)?;
    let granularity = parse_decision_granularity(&req.granularity)?;

    Ok(RespondToDecisionCommand {
        pending_decision_id: PendingDecisionId::from_uuid(uuid),
        action,
        granularity,
    })
}

// --- DomainEvent Conversion ---

pub fn domain_event_to_proto(event: &DomainEvent) -> DomainEventMessage {
    let (event_type, payload_json) = match event {
        DomainEvent::ConnectionDetected(conn) => (
            "connection_detected",
            serde_json::to_string(conn).unwrap_or_default(),
        ),
        DomainEvent::ConnectionUpdated { id, state } => (
            "connection_updated",
            serde_json::json!({ "id": id.as_uuid(), "state": state }).to_string(),
        ),
        DomainEvent::ConnectionClosed(id) => (
            "connection_closed",
            serde_json::json!({ "id": id.as_uuid() }).to_string(),
        ),
        DomainEvent::RuleCreated(rule) => (
            "rule_created",
            serde_json::to_string(rule).unwrap_or_default(),
        ),
        DomainEvent::RuleUpdated(rule) => (
            "rule_updated",
            serde_json::to_string(rule).unwrap_or_default(),
        ),
        DomainEvent::RuleDeleted(id) => (
            "rule_deleted",
            serde_json::json!({ "id": id.as_uuid() }).to_string(),
        ),
        DomainEvent::RuleMatched {
            connection_id,
            rule_id,
            verdict,
        } => (
            "rule_matched",
            serde_json::json!({
                "connection_id": connection_id.as_uuid(),
                "rule_id": rule_id.as_uuid(),
                "verdict": verdict
            })
            .to_string(),
        ),
        DomainEvent::DecisionRequired(pd) => (
            "decision_required",
            serde_json::to_string(pd).unwrap_or_default(),
        ),
        DomainEvent::DecisionResolved(decision) => (
            "decision_resolved",
            serde_json::to_string(decision).unwrap_or_default(),
        ),
        DomainEvent::DecisionExpired(id) => (
            "decision_expired",
            serde_json::json!({ "id": id.as_uuid() }).to_string(),
        ),
        DomainEvent::FirewallStatusChanged(status) => (
            "firewall_status_changed",
            serde_json::to_string(status).unwrap_or_default(),
        ),
        DomainEvent::SystemError { message, severity } => (
            "system_error",
            serde_json::json!({ "message": message, "severity": severity }).to_string(),
        ),
    };

    DomainEventMessage {
        event_type: event_type.to_string(),
        payload_json,
        timestamp: Utc::now().to_rfc3339(),
    }
}

// --- String <-> Enum Helpers ---

fn rule_effect_to_string(effect: RuleEffect) -> String {
    match effect {
        RuleEffect::Allow => "allow".to_string(),
        RuleEffect::Block => "block".to_string(),
        RuleEffect::Ask => "ask".to_string(),
        RuleEffect::Observe => "observe".to_string(),
    }
}

fn parse_rule_effect(s: &str) -> Result<RuleEffect, tonic::Status> {
    match s.to_lowercase().as_str() {
        "allow" => Ok(RuleEffect::Allow),
        "block" => Ok(RuleEffect::Block),
        "ask" => Ok(RuleEffect::Ask),
        "observe" => Ok(RuleEffect::Observe),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule effect: '{}'. Expected: allow, block, ask, observe",
            s
        ))),
    }
}

fn rule_source_to_string(source: RuleSource) -> String {
    match source {
        RuleSource::Manual => "manual".to_string(),
        RuleSource::AutoLearning => "auto_learning".to_string(),
        RuleSource::Import => "import".to_string(),
        RuleSource::System => "system".to_string(),
    }
}

fn parse_rule_source(s: &str) -> Result<RuleSource, tonic::Status> {
    match s.to_lowercase().as_str() {
        "manual" => Ok(RuleSource::Manual),
        "auto_learning" | "autolearning" => Ok(RuleSource::AutoLearning),
        "import" => Ok(RuleSource::Import),
        "system" => Ok(RuleSource::System),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule source: '{}'. Expected: manual, auto_learning, import, system",
            s
        ))),
    }
}

fn pending_status_to_string(status: PendingDecisionStatus) -> String {
    match status {
        PendingDecisionStatus::Pending => "pending".to_string(),
        PendingDecisionStatus::Resolved => "resolved".to_string(),
        PendingDecisionStatus::Expired => "expired".to_string(),
        PendingDecisionStatus::Cancelled => "cancelled".to_string(),
    }
}

fn parse_decision_action(s: &str) -> Result<DecisionAction, tonic::Status> {
    match s.to_lowercase().as_str() {
        "allow_once" | "allowonce" => Ok(DecisionAction::AllowOnce),
        "block_once" | "blockonce" => Ok(DecisionAction::BlockOnce),
        "always_allow" | "alwaysallow" => Ok(DecisionAction::AlwaysAllow),
        "always_block" | "alwaysblock" => Ok(DecisionAction::AlwaysBlock),
        "create_rule" | "createrule" => Ok(DecisionAction::CreateRule),
        "ignore" => Ok(DecisionAction::Ignore),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision action: '{}'. Expected: allow_once, block_once, always_allow, always_block, create_rule, ignore",
            s
        ))),
    }
}

fn parse_decision_granularity(s: &str) -> Result<DecisionGranularity, tonic::Status> {
    match s.to_lowercase().as_str() {
        "app_only" | "apponly" => Ok(DecisionGranularity::AppOnly),
        "app_and_ip" | "appandip" => Ok(DecisionGranularity::AppAndIp),
        "app_and_port" | "appandport" => Ok(DecisionGranularity::AppAndPort),
        "app_and_domain" | "appanddomain" => Ok(DecisionGranularity::AppAndDomain),
        "app_and_protocol" | "appandprotocol" => Ok(DecisionGranularity::AppAndProtocol),
        "full" => Ok(DecisionGranularity::Full),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision granularity: '{}'. Expected: app_only, app_and_ip, app_and_port, app_and_domain, app_and_protocol, full",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule() -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test Rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        }
    }

    fn test_pending_decision() -> PendingDecision {
        PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: ConnectionSnapshot {
                protocol: Protocol::Tcp,
                source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
                destination: SocketAddress::new(
                    "8.8.8.8".parse().unwrap(),
                    Port::new(443).unwrap(),
                ),
                direction: Direction::Outbound,
                process_name: Some("curl".to_string()),
                process_path: None,
                user: Some("seb".to_string()),
            },
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:TCP".to_string(),
            status: PendingDecisionStatus::Pending,
        }
    }

    #[test]
    fn rule_to_proto_roundtrip() {
        let rule = test_rule();
        let proto = rule_to_proto(&rule);
        assert_eq!(proto.id, rule.id.as_uuid().to_string());
        assert_eq!(proto.name, "Test Rule");
        assert_eq!(proto.priority, 10);
        assert!(proto.enabled);
        assert_eq!(proto.effect, "allow");
        assert_eq!(proto.source, "manual");
        assert!(!proto.criteria_json.is_empty());
        assert!(!proto.scope_json.is_empty());
        assert!(!proto.created_at.is_empty());
        assert!(!proto.updated_at.is_empty());
    }

    #[test]
    fn pending_decision_to_proto_roundtrip() {
        let pd = test_pending_decision();
        let proto = pending_decision_to_proto(&pd);
        assert_eq!(proto.id, pd.id.as_uuid().to_string());
        assert_eq!(proto.status, "pending");
        assert!(!proto.snapshot_json.is_empty());
        assert!(!proto.requested_at.is_empty());
        assert!(!proto.expires_at.is_empty());
    }

    #[test]
    fn create_rule_request_valid() {
        let req = CreateRuleRequest {
            name: "Block SSH".to_string(),
            priority: 10,
            criteria_json: "{}".to_string(),
            effect: "block".to_string(),
            scope_json: r#""Permanent""#.to_string(),
            source: "manual".to_string(),
        };
        let cmd = proto_to_create_rule_cmd(&req).unwrap();
        assert_eq!(cmd.name, "Block SSH");
        assert_eq!(cmd.priority, 10);
        assert_eq!(cmd.effect, RuleEffect::Block);
        assert_eq!(cmd.source, RuleSource::Manual);
    }

    #[test]
    fn create_rule_request_empty_name_rejected() {
        let req = CreateRuleRequest {
            name: "".to_string(),
            priority: 10,
            criteria_json: "{}".to_string(),
            effect: "block".to_string(),
            scope_json: r#""Permanent""#.to_string(),
            source: "manual".to_string(),
        };
        let result = proto_to_create_rule_cmd(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn create_rule_request_invalid_json_rejected() {
        let req = CreateRuleRequest {
            name: "Test".to_string(),
            priority: 10,
            criteria_json: "not json".to_string(),
            effect: "block".to_string(),
            scope_json: r#""Permanent""#.to_string(),
            source: "manual".to_string(),
        };
        let result = proto_to_create_rule_cmd(&req);
        assert!(result.is_err());
    }

    #[test]
    fn respond_cmd_valid() {
        let uuid = Uuid::new_v4();
        let req = DecisionResponseRequest {
            pending_decision_id: uuid.to_string(),
            action: "allow_once".to_string(),
            granularity: "app_only".to_string(),
        };
        let cmd = proto_to_respond_cmd(&req).unwrap();
        assert_eq!(*cmd.pending_decision_id.as_uuid(), uuid);
        assert_eq!(cmd.action, DecisionAction::AllowOnce);
        assert_eq!(cmd.granularity, DecisionGranularity::AppOnly);
    }

    #[test]
    fn respond_cmd_invalid_uuid_rejected() {
        let req = DecisionResponseRequest {
            pending_decision_id: "not-a-uuid".to_string(),
            action: "allow_once".to_string(),
            granularity: "app_only".to_string(),
        };
        let result = proto_to_respond_cmd(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn respond_cmd_invalid_action_rejected() {
        let req = DecisionResponseRequest {
            pending_decision_id: Uuid::new_v4().to_string(),
            action: "invalid_action".to_string(),
            granularity: "app_only".to_string(),
        };
        let result = proto_to_respond_cmd(&req);
        assert!(result.is_err());
    }

    #[test]
    fn domain_event_to_proto_rule_created() {
        let rule = test_rule();
        let event = DomainEvent::RuleCreated(rule.clone());
        let proto = domain_event_to_proto(&event);
        assert_eq!(proto.event_type, "rule_created");
        assert!(!proto.payload_json.is_empty());
        assert!(!proto.timestamp.is_empty());
    }

    #[test]
    fn domain_event_to_proto_decision_required() {
        let pd = test_pending_decision();
        let event = DomainEvent::DecisionRequired(pd);
        let proto = domain_event_to_proto(&event);
        assert_eq!(proto.event_type, "decision_required");
    }

    #[test]
    fn domain_event_to_proto_system_error() {
        let event = DomainEvent::SystemError {
            message: "test error".to_string(),
            severity: Severity::Error,
        };
        let proto = domain_event_to_proto(&event);
        assert_eq!(proto.event_type, "system_error");
        assert!(proto.payload_json.contains("test error"));
    }

    #[test]
    fn error_mapping_variants() {
        let validation = domain_error_to_status(DomainError::Validation("bad".into()));
        assert_eq!(validation.code(), tonic::Code::InvalidArgument);

        let not_found = domain_error_to_status(DomainError::NotFound("gone".into()));
        assert_eq!(not_found.code(), tonic::Code::NotFound);

        let exists = domain_error_to_status(DomainError::AlreadyExists("dup".into()));
        assert_eq!(exists.code(), tonic::Code::AlreadyExists);

        let infra = domain_error_to_status(DomainError::Infrastructure("db".into()));
        assert_eq!(infra.code(), tonic::Code::Internal);

        let perm = domain_error_to_status(DomainError::NotPermitted("no".into()));
        assert_eq!(perm.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn parse_rule_effect_all_variants() {
        assert_eq!(parse_rule_effect("allow").unwrap(), RuleEffect::Allow);
        assert_eq!(parse_rule_effect("BLOCK").unwrap(), RuleEffect::Block);
        assert_eq!(parse_rule_effect("Ask").unwrap(), RuleEffect::Ask);
        assert_eq!(parse_rule_effect("observe").unwrap(), RuleEffect::Observe);
        assert!(parse_rule_effect("invalid").is_err());
    }

    #[test]
    fn parse_decision_action_all_variants() {
        assert_eq!(parse_decision_action("allow_once").unwrap(), DecisionAction::AllowOnce);
        assert_eq!(parse_decision_action("block_once").unwrap(), DecisionAction::BlockOnce);
        assert_eq!(parse_decision_action("always_allow").unwrap(), DecisionAction::AlwaysAllow);
        assert_eq!(parse_decision_action("always_block").unwrap(), DecisionAction::AlwaysBlock);
        assert_eq!(parse_decision_action("create_rule").unwrap(), DecisionAction::CreateRule);
        assert_eq!(parse_decision_action("ignore").unwrap(), DecisionAction::Ignore);
        assert!(parse_decision_action("bad").is_err());
    }

    #[test]
    fn parse_decision_granularity_all_variants() {
        assert_eq!(parse_decision_granularity("app_only").unwrap(), DecisionGranularity::AppOnly);
        assert_eq!(parse_decision_granularity("app_and_ip").unwrap(), DecisionGranularity::AppAndIp);
        assert_eq!(parse_decision_granularity("full").unwrap(), DecisionGranularity::Full);
        assert!(parse_decision_granularity("bad").is_err());
    }
}
```

- [ ] **Step 2: Update grpc/mod.rs to declare the converters module**

Replace the contents of `crates/daemon/src/grpc/mod.rs` with:
```rust
pub mod converters;
```

- [ ] **Step 3: Verify converters compile and tests pass**

```bash
cd /home/seb/Dev/SysWall && cargo test -p syswall-daemon -- grpc::converters
```

---

### Task 3: gRPC Control Service

**Files:**
- Create: `crates/daemon/src/grpc/control_service.rs`
- Modify: `crates/daemon/src/grpc/mod.rs`

- [ ] **Step 1: Create control_service.rs**

`crates/daemon/src/grpc/control_service.rs`:
```rust
use std::sync::Arc;

use tonic::{Request, Response, Status};

use syswall_app::services::learning_service::LearningService;
use syswall_app::services::rule_service::RuleService;
use syswall_domain::entities::RuleId;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{FirewallEngine, RuleFilters};
use syswall_proto::syswall::sys_wall_control_server::SysWallControl;
use syswall_proto::syswall::{
    CreateRuleRequest, DecisionAck, DecisionResponseRequest, Empty, PendingDecisionListResponse,
    RuleFiltersRequest, RuleIdRequest, RuleListResponse, RuleResponse, StatusResponse,
    ToggleRuleRequest,
};
use uuid::Uuid;

use super::converters::{
    domain_error_to_status, pending_decision_to_proto, proto_to_create_rule_cmd,
    proto_to_respond_cmd, rule_to_proto,
};

pub struct SysWallControlService {
    rule_service: Arc<RuleService>,
    learning_service: Arc<LearningService>,
    firewall: Arc<dyn FirewallEngine>,
}

impl SysWallControlService {
    pub fn new(
        rule_service: Arc<RuleService>,
        learning_service: Arc<LearningService>,
        firewall: Arc<dyn FirewallEngine>,
    ) -> Self {
        Self {
            rule_service,
            learning_service,
            firewall,
        }
    }
}

#[tonic::async_trait]
impl SysWallControl for SysWallControlService {
    async fn get_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<StatusResponse>, Status> {
        let status = self
            .firewall
            .get_status()
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(StatusResponse {
            enabled: status.enabled,
            active_rules_count: status.active_rules_count,
            nftables_synced: status.nftables_synced,
            uptime_secs: status.uptime_secs,
            version: status.version,
        }))
    }

    async fn list_rules(
        &self,
        request: Request<RuleFiltersRequest>,
    ) -> Result<Response<RuleListResponse>, Status> {
        let req = request.into_inner();
        let pagination = Pagination {
            offset: req.offset,
            limit: if req.limit == 0 { 50 } else { req.limit },
        };

        let rules = self
            .rule_service
            .list_rules(&RuleFilters::default(), &pagination)
            .await
            .map_err(domain_error_to_status)?;

        let rule_messages = rules.iter().map(rule_to_proto).collect();

        Ok(Response::new(RuleListResponse {
            rules: rule_messages,
        }))
    }

    async fn create_rule(
        &self,
        request: Request<CreateRuleRequest>,
    ) -> Result<Response<RuleResponse>, Status> {
        let req = request.into_inner();
        let cmd = proto_to_create_rule_cmd(&req)?;

        let rule = self
            .rule_service
            .create_rule(cmd)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(RuleResponse {
            rule: Some(rule_to_proto(&rule)),
        }))
    }

    async fn delete_rule(
        &self,
        request: Request<RuleIdRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        let uuid = Uuid::parse_str(&req.id)
            .map_err(|e| Status::invalid_argument(format!("Invalid rule ID: {}", e)))?;
        let rule_id = RuleId::from_uuid(uuid);

        self.rule_service
            .delete_rule(&rule_id)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(Empty {}))
    }

    async fn toggle_rule(
        &self,
        request: Request<ToggleRuleRequest>,
    ) -> Result<Response<RuleResponse>, Status> {
        let req = request.into_inner();
        let uuid = Uuid::parse_str(&req.id)
            .map_err(|e| Status::invalid_argument(format!("Invalid rule ID: {}", e)))?;
        let rule_id = RuleId::from_uuid(uuid);

        let rule = self
            .rule_service
            .toggle_rule(&rule_id, req.enabled)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(RuleResponse {
            rule: Some(rule_to_proto(&rule)),
        }))
    }

    async fn respond_to_decision(
        &self,
        request: Request<DecisionResponseRequest>,
    ) -> Result<Response<DecisionAck>, Status> {
        let req = request.into_inner();
        let cmd = proto_to_respond_cmd(&req)?;

        let decision = self
            .learning_service
            .resolve_decision(cmd)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(DecisionAck {
            decision_id: decision.id.as_uuid().to_string(),
        }))
    }

    async fn list_pending_decisions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<PendingDecisionListResponse>, Status> {
        let pending = self
            .learning_service
            .get_pending_decisions()
            .await
            .map_err(domain_error_to_status)?;

        let decision_messages = pending.iter().map(pending_decision_to_proto).collect();

        Ok(Response::new(PendingDecisionListResponse {
            decisions: decision_messages,
        }))
    }
}
```

- [ ] **Step 2: Add control_service module to grpc/mod.rs**

Update `crates/daemon/src/grpc/mod.rs`:
```rust
pub mod control_service;
pub mod converters;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 4: gRPC Event Service

**Files:**
- Create: `crates/daemon/src/grpc/event_service.rs`
- Modify: `crates/daemon/src/grpc/mod.rs`

- [ ] **Step 1: Create event_service.rs**

`crates/daemon/src/grpc/event_service.rs`:
```rust
use std::pin::Pin;
use std::sync::Arc;

use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_domain::ports::EventBus;
use syswall_proto::syswall::sys_wall_events_server::SysWallEvents;
use syswall_proto::syswall::{DomainEventMessage, SubscribeRequest};

use super::converters::domain_event_to_proto;

pub struct SysWallEventService {
    event_bus: Arc<TokioBroadcastEventBus>,
}

impl SysWallEventService {
    pub fn new(event_bus: Arc<TokioBroadcastEventBus>) -> Self {
        Self { event_bus }
    }
}

#[tonic::async_trait]
impl SysWallEvents for SysWallEventService {
    type SubscribeEventsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<DomainEventMessage, Status>> + Send>>;

    async fn subscribe_events(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let receiver = self.event_bus.subscribe();
        let stream = BroadcastStream::new(receiver);

        let mapped = stream.filter_map(|result| match result {
            Ok(event) => Some(Ok(domain_event_to_proto(&event))),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                tracing::warn!("Event stream subscriber lagged, missed {} events", n);
                None // Skip lagged errors, continue streaming
            }
        });

        Ok(Response::new(Box::pin(mapped)))
    }
}
```

- [ ] **Step 2: Add event_service module to grpc/mod.rs**

Update `crates/daemon/src/grpc/mod.rs`:
```rust
pub mod control_service;
pub mod converters;
pub mod event_service;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 5: gRPC Server (Unix Socket)

**Files:**
- Create: `crates/daemon/src/grpc/server.rs`
- Modify: `crates/daemon/src/grpc/mod.rs`

- [ ] **Step 1: Create server.rs with Unix socket transport**

`crates/daemon/src/grpc/server.rs`:
```rust
use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::{info, warn};

use syswall_proto::syswall::sys_wall_control_server::SysWallControlServer;
use syswall_proto::syswall::sys_wall_events_server::SysWallEventsServer;

use super::control_service::SysWallControlService;
use super::event_service::SysWallEventService;

/// Start the gRPC server on a Unix domain socket.
pub async fn start_grpc_server(
    socket_path: PathBuf,
    control_service: SysWallControlService,
    event_service: SysWallEventService,
    cancel: CancellationToken,
) -> Result<(), String> {
    // Remove existing socket file if present
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .map_err(|e| format!("Failed to remove old socket: {}", e))?;
    }

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket directory: {}", e))?;
    }

    // Bind Unix listener
    let listener = UnixListener::bind(&socket_path)
        .map_err(|e| format!("Failed to bind Unix socket at {:?}: {}", socket_path, e))?;

    info!("gRPC server listening on {:?}", socket_path);

    // Set socket permissions to 0660 (owner + group read/write)
    set_socket_permissions(&socket_path);

    let stream = UnixListenerStream::new(listener);

    // Build tonic server with both services
    let server = Server::builder()
        .add_service(SysWallControlServer::new(control_service))
        .add_service(SysWallEventsServer::new(event_service));

    // Run server with graceful shutdown on cancellation
    server
        .serve_with_incoming_shutdown(stream, cancel.cancelled())
        .await
        .map_err(|e| format!("gRPC server error: {}", e))?;

    // Clean up socket file on shutdown
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    info!("gRPC server stopped");
    Ok(())
}

/// Set socket permissions to 0660 and group to syswall (best-effort).
fn set_socket_permissions(socket_path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    // Set permissions to 0660
    if let Err(e) = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660)) {
        warn!("Failed to set socket permissions: {}", e);
    }

    // Try to set group to syswall
    match nix::unistd::Group::from_name("syswall") {
        Ok(Some(group)) => {
            if let Err(e) = nix::unistd::chown(socket_path.as_path(), None, Some(group.gid)) {
                warn!("Failed to set socket group to syswall: {}", e);
            }
        }
        Ok(None) => {
            warn!("Group 'syswall' not found, socket will use default group");
        }
        Err(e) => {
            warn!("Failed to look up syswall group: {}", e);
        }
    }
}
```

- [ ] **Step 2: Add server module to grpc/mod.rs**

Update `crates/daemon/src/grpc/mod.rs`:
```rust
pub mod control_service;
pub mod converters;
pub mod event_service;
pub mod server;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 6: Daemon Integration (Supervisor Tasks)

**Files:**
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Add gRPC server and decision-expiry tasks to supervisor**

In `crates/daemon/src/main.rs`, add the following after the connection-monitor supervisor task and before the `info!("SysWall daemon ready")` line:

```rust
    // gRPC server
    supervisor.spawn("grpc-server", {
        let rule_service = ctx.rule_service.clone();
        let learning_service = ctx.learning_service.clone();
        let firewall = ctx.firewall.clone();
        let event_bus = ctx.event_bus.clone();
        let socket_path = config.daemon.socket_path.clone();
        let cancel = cancel.clone();

        async move {
            let control_service = grpc::control_service::SysWallControlService::new(
                rule_service,
                learning_service.clone(),
                firewall,
            );
            let event_service = grpc::event_service::SysWallEventService::new(event_bus);

            grpc::server::start_grpc_server(socket_path, control_service, event_service, cancel)
                .await
        }
    });

    // Periodic decision expiration
    supervisor.spawn("decision-expiry", {
        let learning_service = ctx.learning_service.clone();
        let cancel = cancel.clone();
        async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                        match learning_service.expire_overdue().await {
                            Ok(expired) if !expired.is_empty() => {
                                info!("Expired {} overdue pending decisions", expired.len());
                            }
                            Err(e) => {
                                warn!("Decision expiry error: {}", e);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        }
    });
```

Also add the required use statement at the top of main.rs (if not already present):
```rust
use std::time::Duration;
```

- [ ] **Step 2: Verify the full daemon compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

- [ ] **Step 3: Run all tests across the workspace**

```bash
cd /home/seb/Dev/SysWall && cargo test --workspace
```

---

### Task 7: Fix Compilation Issues and Finalize

**Files:**
- Any files from previous tasks that need adjustments

This task is a catch-all for resolving any compilation issues discovered during the build step. Common issues to watch for:

- [ ] **Step 1: Resolve any import path issues**

The `tokio-stream` crate must export `wrappers::UnixListenerStream` -- ensure the daemon's `tokio-stream` dependency has the `net` feature:

In `Cargo.toml` workspace dependencies, update if needed:
```toml
tokio-stream = { version = "0.1", features = ["net"] }
```

- [ ] **Step 2: Verify tonic server supports `serve_with_incoming_shutdown` with UnixListenerStream**

The tonic `Server::serve_with_incoming_shutdown()` method accepts any `Stream<Item = Result<impl AsyncRead + AsyncWrite, _>>`. The `UnixListenerStream` from `tokio-stream` produces `Result<UnixStream, io::Error>`, which satisfies this bound. If tonic version 0.12 has a different API shape, adapt accordingly (e.g., use `tonic::transport::server::Router::serve_with_incoming_shutdown`).

- [ ] **Step 3: Ensure serde_json feature is available in daemon**

The converters use `serde_json::json!()` macro and `serde_json::to_string()`. Verify these compile. Also ensure `syswall_domain` entities derive `Serialize` (they already do).

- [ ] **Step 4: Final full workspace test**

```bash
cd /home/seb/Dev/SysWall && cargo test --workspace
```

---

### Summary

| Task | Description | New Files | Modified Files |
|---|---|---|---|
| 1 | Add Dependencies | -- | `Cargo.toml`, `crates/daemon/Cargo.toml` |
| 2 | Proto <-> Domain Converters | `grpc/converters.rs` | `grpc/mod.rs` |
| 3 | gRPC Control Service | `grpc/control_service.rs` | `grpc/mod.rs` |
| 4 | gRPC Event Service | `grpc/event_service.rs` | `grpc/mod.rs` |
| 5 | gRPC Server (Unix Socket) | `grpc/server.rs` | `grpc/mod.rs` |
| 6 | Daemon Integration | -- | `main.rs` |
| 7 | Fix Compilation & Finalize | -- | Any as needed |

**Total: 7 tasks, 4 new files, ~4 modified files**
