use std::collections::HashSet;
use std::ffi::CString;
use std::sync::Arc;

use nix::unistd::{Uid, User, getgrouplist};
use tokio::sync::mpsc;
use tonic::{Request, Status, service::Interceptor};

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};

/// Peer credentials extracted from the Unix socket via SO_PEERCRED.
/// Identifiants du peer extraits du socket Unix via SO_PEERCRED.
#[derive(Debug, Clone, Copy)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    pub pid: i32,
}

/// Résout l'ensemble des gid (primaire + supplémentaires) d'un utilisateur.
/// Injectable pour tester l'appartenance aux groupes sans dépendre du NSS réel.
/// Resolves the set of gids (primary + supplementary) of a user.
/// Injectable so group membership can be tested without touching the real NSS.
type UserGidsResolver = Arc<dyn Fn(u32) -> HashSet<u32> + Send + Sync>;

/// Résolveur de production : interroge le NSS (getpwuid + getgrouplist).
/// Production resolver: queries NSS (getpwuid + getgrouplist).
fn nss_user_gids(uid: u32) -> HashSet<u32> {
    let mut gids = HashSet::new();
    if let Ok(Some(user)) = User::from_uid(Uid::from_raw(uid)) {
        gids.insert(user.gid.as_raw());
        if let Ok(cname) = CString::new(user.name.as_bytes()) {
            if let Ok(groups) = getgrouplist(&cname, user.gid) {
                gids.extend(groups.into_iter().map(|g| g.as_raw()));
            }
        }
    }
    gids
}

/// Allowed identities for gRPC calls.
/// Identités autorisées pour les appels gRPC.
#[derive(Clone)]
pub struct PeerAuthPolicy {
    pub allowed_uids: HashSet<u32>,
    pub allowed_gids: HashSet<u32>,
    // SO_PEERCRED ne renvoie que le gid *primaire* ; pour autoriser un membre du
    // groupe syswall qui ne l'a pas en gid primaire (cas d'une UI lancee normalement,
    // gid primaire = groupe personnel), on consulte aussi ses groupes supplementaires.
    // SO_PEERCRED only reports the *primary* gid; to allow a syswall group member that
    // does not have it as primary gid (a UI launched normally, primary gid = personal
    // group), we also consult its supplementary groups.
    user_gids: UserGidsResolver,
}

impl std::fmt::Debug for PeerAuthPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerAuthPolicy")
            .field("allowed_uids", &self.allowed_uids)
            .field("allowed_gids", &self.allowed_gids)
            .finish_non_exhaustive()
    }
}

impl PeerAuthPolicy {
    pub fn new(allowed_uids: HashSet<u32>, allowed_gids: HashSet<u32>) -> Self {
        Self {
            allowed_uids,
            allowed_gids,
            user_gids: Arc::new(nss_user_gids),
        }
    }

    /// Variante avec résolveur de groupes injecté (pour les tests unitaires).
    /// Variant with an injected group resolver (for unit tests).
    pub fn with_resolver(
        allowed_uids: HashSet<u32>,
        allowed_gids: HashSet<u32>,
        user_gids: UserGidsResolver,
    ) -> Self {
        Self {
            allowed_uids,
            allowed_gids,
            user_gids,
        }
    }

    pub fn permits(&self, creds: &PeerCredentials) -> bool {
        // Chemins rapides : uid explicitement autorisé, ou gid primaire autorisé.
        // Fast paths: explicitly allowed uid, or allowed primary gid.
        if self.allowed_uids.contains(&creds.uid) || self.allowed_gids.contains(&creds.gid) {
            return true;
        }
        // Sinon : membre *supplémentaire* d'un groupe autorisé (ex. utilisateur dans le
        // groupe syswall sans l'avoir en gid primaire).
        // Otherwise: *supplementary* member of an allowed group (e.g. a user in the
        // syswall group without it as primary gid).
        !self.allowed_gids.is_disjoint(&(self.user_gids)(creds.uid))
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
                format!(
                    "gRPC denied: uid={} gid={} pid={}",
                    creds.uid, creds.gid, creds.pid
                ),
            )
            .with_metadata("uid", creds.uid.to_string())
            .with_metadata("gid", creds.gid.to_string())
            .with_metadata("pid", creds.pid.to_string());
            if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
                self.audit_tx.try_send(event)
            {
                tracing::warn!(target: "auth", "audit channel full, denial event dropped");
            }
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
        let req = make(PeerCredentials {
            uid: 0,
            gid: 100,
            pid: 9,
        });
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
        let req = make(PeerCredentials {
            uid: 1000,
            gid: 1234,
            pid: 9,
        });
        assert!(intercept.call(req).is_ok());
    }

    #[test]
    fn unprivileged_user_denied_and_audited() {
        let (tx, mut rx) = audit_pair();
        // Résolveur vide : l'utilisateur n'appartient à aucun groupe autorisé.
        // Empty resolver: the user belongs to no allowed group.
        let policy = Arc::new(PeerAuthPolicy::with_resolver(
            HashSet::from([0]),
            HashSet::from([1234]),
            Arc::new(|_uid| HashSet::new()),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials {
            uid: 1000,
            gid: 1000,
            pid: 9,
        });
        let err = intercept.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
        let event = rx.try_recv().expect("audit event emitted on denial");
        assert_eq!(event.category, EventCategory::Authentication);
        assert_eq!(event.severity, Severity::Warning);
        assert_eq!(event.metadata.get("uid").unwrap(), "1000");
    }

    #[test]
    fn supplementary_group_member_is_allowed() {
        // Cas réel : UI lancée normalement, gid primaire = groupe personnel (1001),
        // mais l'utilisateur est membre *supplémentaire* du groupe syswall (1234).
        // SO_PEERCRED ne voit que 1001 ; la résolution des groupes doit l'autoriser.
        // Real case: UI launched normally, primary gid = personal group (1001), but the
        // user is a *supplementary* member of the syswall group (1234). SO_PEERCRED only
        // sees 1001; group resolution must still allow it.
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::with_resolver(
            HashSet::from([0]),
            HashSet::from([1234]),
            Arc::new(|uid| {
                if uid == 1000 {
                    HashSet::from([1001, 1234])
                } else {
                    HashSet::new()
                }
            }),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials {
            uid: 1000,
            gid: 1001,
            pid: 9,
        });
        assert!(intercept.call(req).is_ok());
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
