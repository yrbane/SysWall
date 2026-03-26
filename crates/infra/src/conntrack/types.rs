use std::net::IpAddr;

use syswall_domain::value_objects::Protocol;

/// The type of conntrack event.
/// Le type d'evenement conntrack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConntrackEventType {
    New,
    Update,
    Destroy,
}

/// A parsed conntrack event (raw, before domain transformation).
/// Un evenement conntrack parse (brut, avant transformation en domaine).
#[derive(Debug, Clone)]
pub struct ConntrackEvent {
    pub timestamp: f64,
    pub event_type: ConntrackEventType,
    pub protocol: Protocol,
    pub proto_number: u8,
    pub state: Option<String>,
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
    pub reply_src: Option<IpAddr>,
    pub reply_dst: Option<IpAddr>,
    pub reply_sport: Option<u16>,
    pub reply_dport: Option<u16>,
}
