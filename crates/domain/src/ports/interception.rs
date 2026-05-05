use async_trait::async_trait;
use std::sync::Arc;

use crate::entities::Connection;
use crate::errors::DomainError;

/// Verdict to apply to a captured packet.
/// Verdict à appliquer à un paquet capturé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketVerdict {
    /// Allow the packet through.
    /// Laisser passer le paquet.
    Accept,
    /// Drop the packet (kernel-side).
    /// Jeter le paquet (côté kernel).
    Drop,
}

/// Handler resolving a verdict for a captured connection.
/// Handler résolvant un verdict pour une connexion capturée.
#[async_trait]
pub trait PacketDecisionHandler: Send + Sync {
    async fn decide(&self, connection: &Connection) -> Result<PacketVerdict, DomainError>;
}

/// Intercepts the first packet of every new flow and asks for a verdict.
/// Intercepte le premier paquet de chaque nouveau flux et demande un verdict.
#[async_trait]
pub trait PacketInterceptor: Send + Sync {
    /// Run the interception loop until the cancel token fires.
    /// Lance la boucle d'interception jusqu'au déclenchement du cancel token.
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_equality() {
        assert_eq!(PacketVerdict::Accept, PacketVerdict::Accept);
        assert_ne!(PacketVerdict::Accept, PacketVerdict::Drop);
    }
}
