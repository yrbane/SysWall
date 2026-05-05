use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::{service::Interceptor, Request, Status};

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};

/// Peer credentials extracted from the Unix socket via SO_PEERCRED.
/// Identifiants du peer extraits du socket Unix via SO_PEERCRED.
#[derive(Debug, Clone, Copy)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    pub pid: i32,
}

/// Allowed identities for gRPC calls.
/// Identités autorisées pour les appels gRPC.
#[derive(Debug, Clone)]
pub struct PeerAuthPolicy {
    pub allowed_uids: HashSet<u32>,
    pub allowed_gids: HashSet<u32>,
}

impl PeerAuthPolicy {
    pub fn new(allowed_uids: HashSet<u32>, allowed_gids: HashSet<u32>) -> Self {
        Self { allowed_uids, allowed_gids }
    }

    pub fn permits(&self, creds: &PeerCredentials) -> bool {
        self.allowed_uids.contains(&creds.uid) || self.allowed_gids.contains(&creds.gid)
    }
}

/// Tonic interceptor that gates each gRPC call on peer credentials.
/// Interceptor tonic qui filtre chaque appel gRPC sur les identifiants peer.
#[derive(Clone)]
pub struct PeerAuthInterceptor {
    policy: Arc<PeerAuthPolicy>,
    audit_tx: mpsc::Sender<AuditEvent>,
}

impl PeerAuthInterceptor {
    pub fn new(policy: Arc<PeerAuthPolicy>, audit_tx: mpsc::Sender<AuditEvent>) -> Self {
        Self { policy, audit_tx }
    }
}

impl Interceptor for PeerAuthInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let creds = req
            .extensions()
            .get::<PeerCredentials>()
            .copied()
            .ok_or_else(|| Status::internal("peer credentials unavailable"))?;
        if self.policy.permits(&creds) {
            Ok(req)
        } else {
            let event = AuditEvent::new(
                Severity::Warning,
                EventCategory::Authentication,
                format!("gRPC denied: uid={} gid={} pid={}", creds.uid, creds.gid, creds.pid),
            )
            .with_metadata("uid", creds.uid.to_string())
            .with_metadata("gid", creds.gid.to_string())
            .with_metadata("pid", creds.pid.to_string());
            let _ = self.audit_tx.try_send(event);
            Err(Status::permission_denied(
                "syswall: caller must be root or in group 'syswall'",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(creds: PeerCredentials) -> Request<()> {
        let mut req = Request::new(());
        req.extensions_mut().insert(creds);
        req
    }

    fn audit_pair() -> (mpsc::Sender<AuditEvent>, mpsc::Receiver<AuditEvent>) {
        mpsc::channel(8)
    }

    #[test]
    fn root_uid_is_allowed() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 0, gid: 100, pid: 9 });
        assert!(intercept.call(req).is_ok());
    }

    #[test]
    fn syswall_gid_is_allowed() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 1000, gid: 1234, pid: 9 });
        assert!(intercept.call(req).is_ok());
    }

    #[test]
    fn unprivileged_user_denied_and_audited() {
        let (tx, mut rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 1000, gid: 1000, pid: 9 });
        let err = intercept.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
        let event = rx.try_recv().expect("audit event emitted on denial");
        assert_eq!(event.category, EventCategory::Authentication);
        assert_eq!(event.severity, Severity::Warning);
        assert_eq!(event.metadata.get("uid").unwrap(), "1000");
    }

    #[test]
    fn missing_credentials_returns_internal() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(HashSet::from([0]), HashSet::new()));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = Request::new(());
        let err = intercept.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Internal);
    }
}
