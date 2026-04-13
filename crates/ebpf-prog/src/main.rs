#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::{bpf_get_current_comm, bpf_get_current_pid_tgid},
    macros::{map, tracepoint},
    maps::RingBuf,
    programs::TracePointContext,
};

/// Événement socket poussé vers l'espace utilisateur via le ring buffer.
/// Socket event pushed to userspace via ring buffer.
#[repr(C)]
pub struct SocketEvent {
    pub pid: u32,
    pub tgid: u32,
    pub comm: [u8; 16],
    pub sport: u16,
    pub dport: u16,
    pub family: u16,  // AF_INET=2, AF_INET6=10
    pub protocol: u8, // IPPROTO_TCP=6, IPPROTO_UDP=17
    pub _pad: u8,
    pub oldstate: u32,
    pub newstate: u32,
}

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

/// Point de trace : sock/inet_sock_set_state
/// Se déclenche à chaque transition d'état TCP (SYN_SENT, ESTABLISHED, CLOSE, etc.)
///
/// Tracepoint: sock/inet_sock_set_state
/// Fires on every TCP state transition.
#[tracepoint]
pub fn inet_sock_set_state(ctx: TracePointContext) -> u32 {
    match try_inet_sock_set_state(&ctx) {
        Ok(ret) => ret,
        Err(_) => 1,
    }
}

fn try_inet_sock_set_state(ctx: &TracePointContext) -> Result<u32, i64> {
    // Lecture des champs du tracepoint depuis le contexte
    // Read tracepoint fields from context
    //
    // struct trace_event_raw_inet_sock_set_state layout (offsets):
    // __data: 8 bytes header
    // oldstate: u32 at +8
    // newstate: u32 at +12
    // sport: u16 at +16 (network byte order)
    // dport: u16 at +18 (network byte order)
    // family: u16 at +20
    // protocol: u8 at +22
    let oldstate: u32 = unsafe { ctx.read_at(8)? };
    let newstate: u32 = unsafe { ctx.read_at(12)? };
    let sport: u16 = unsafe { ctx.read_at(16)? };
    let dport: u16 = unsafe { ctx.read_at(18)? };
    let family: u16 = unsafe { ctx.read_at(20)? };
    let protocol: u8 = unsafe { ctx.read_at(22)? };

    // Filtrage : uniquement IPv4 et IPv6
    // Filter: only IPv4 and IPv6
    if family != 2 && family != 10 {
        return Ok(0);
    }

    let pid_tgid = bpf_get_current_pid_tgid();
    let pid = (pid_tgid >> 32) as u32;
    let tgid = pid_tgid as u32;

    let comm = match bpf_get_current_comm() {
        Ok(c) => c,
        Err(_) => [0u8; 16],
    };

    // Réserve de l'espace dans le ring buffer et écriture de l'événement
    // Reserve space in ring buffer and write event
    if let Some(mut buf) = EVENTS.reserve::<SocketEvent>(0) {
        let event = buf.as_mut_ptr();
        unsafe {
            (*event).pid = pid;
            (*event).tgid = tgid;
            (*event).comm = comm;
            (*event).sport = sport;
            (*event).dport = dport;
            (*event).family = family;
            (*event).protocol = protocol;
            (*event)._pad = 0;
            (*event).oldstate = oldstate;
            (*event).newstate = newstate;
        }
        buf.submit(0);
    }

    Ok(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
