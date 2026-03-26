/// gRPC control service implementing the SysWallControl trait.
/// Service de contrôle gRPC implémentant le trait SysWallControl.

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

use super::converters::{
    domain_error_to_status, pending_decision_to_proto, proto_to_create_rule_cmd,
    proto_to_respond_cmd, rule_to_proto, status_to_proto,
};

/// Control service holding Arc references to the app services.
/// Service de contrôle détenant des références Arc vers les services applicatifs.
pub struct SysWallControlService {
    rule_service: Arc<RuleService>,
    learning_service: Arc<LearningService>,
    firewall: Arc<dyn FirewallEngine>,
}

impl SysWallControlService {
    /// Create a new control service instance.
    /// Crée une nouvelle instance du service de contrôle.
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

        Ok(Response::new(status_to_proto(&status)))
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
        let uuid = uuid::Uuid::parse_str(&req.id)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;
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
        let uuid = uuid::Uuid::parse_str(&req.id)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;
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
        let decisions = self
            .learning_service
            .get_pending_decisions()
            .await
            .map_err(domain_error_to_status)?;

        let decision_messages = decisions.iter().map(pending_decision_to_proto).collect();

        Ok(Response::new(PendingDecisionListResponse {
            decisions: decision_messages,
        }))
    }
}
