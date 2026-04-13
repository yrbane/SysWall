/// Événement socket reçu du ring buffer BPF.
/// Doit correspondre au layout dans crates/ebpf-prog/src/main.rs.
///
/// Socket event received from the BPF ring buffer.
/// Must match the layout in crates/ebpf-prog/src/main.rs.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SocketEvent {
    pub pid: u32,
    pub tgid: u32,
    pub comm: [u8; 16],
    pub sport: u16,
    pub dport: u16,
    pub family: u16,
    pub protocol: u8,
    pub _pad: u8,
    pub oldstate: u32,
    pub newstate: u32,
}

impl SocketEvent {
    /// Extrait le nom de commande du processus en tant que chaîne.
    /// Extract the process command name as a string.
    pub fn comm_str(&self) -> &str {
        let len = self.comm.iter().position(|&b| b == 0).unwrap_or(16);
        std::str::from_utf8(&self.comm[..len]).unwrap_or("<invalid>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comm_str_extracts_name() {
        let mut event = SocketEvent {
            pid: 1,
            tgid: 1,
            comm: [0u8; 16],
            sport: 80,
            dport: 443,
            family: 2,
            protocol: 6,
            _pad: 0,
            oldstate: 0,
            newstate: 1,
        };
        event.comm[..7].copy_from_slice(b"firefox");
        assert_eq!(event.comm_str(), "firefox");
    }

    #[test]
    fn comm_str_handles_full_buffer() {
        let event = SocketEvent {
            pid: 1,
            tgid: 1,
            comm: *b"0123456789abcdef",
            sport: 80,
            dport: 443,
            family: 2,
            protocol: 6,
            _pad: 0,
            oldstate: 0,
            newstate: 1,
        };
        assert_eq!(event.comm_str(), "0123456789abcdef");
    }
}
