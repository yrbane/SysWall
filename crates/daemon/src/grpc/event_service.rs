/// gRPC event streaming service implementing the SysWallEvents trait.
/// Service de streaming d'événements gRPC implémentant le trait SysWallEvents.

use std::sync::Arc;

use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::warn;

use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_domain::ports::EventBus;
use syswall_proto::syswall::sys_wall_events_server::SysWallEvents;
use syswall_proto::syswall::{DomainEventMessage, SubscribeRequest};

use super::converters::domain_event_to_proto;

/// Event streaming service backed by the domain event bus.
/// Service de streaming d'événements adossé au bus d'événements du domaine.
pub struct SysWallEventService {
    event_bus: Arc<TokioBroadcastEventBus>,
}

impl SysWallEventService {
    /// Create a new event service instance.
    /// Crée une nouvelle instance du service d'événements.
    pub fn new(event_bus: Arc<TokioBroadcastEventBus>) -> Self {
        Self { event_bus }
    }
}

#[tonic::async_trait]
impl SysWallEvents for SysWallEventService {
    type SubscribeEventsStream =
        std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<DomainEventMessage, Status>> + Send + 'static>>;

    async fn subscribe_events(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let receiver = self.event_bus.subscribe();
        let stream = BroadcastStream::new(receiver);

        let mapped = stream.filter_map(|result| match result {
            Ok(event) => Some(Ok(domain_event_to_proto(&event))),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                warn!("Event stream lagged, missed {} events", n);
                None
            }
        });

        Ok(Response::new(Box::pin(mapped)))
    }
}
