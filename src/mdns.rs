//! Multicast DNS (mDNS) responder for device discovery on local networks.

use embassy_net::Ipv4Address;

/// Default hostname advertised over mDNS (resolves to khvylyna.local).
pub const MDNS_HOSTNAME: &str = "khvylyna";

/// Multicast DNS standard IPv4 destination address: 224.0.0.251.
pub const MDNS_MCAST_IPV4: Ipv4Address = Ipv4Address::new(224, 0, 0, 251);

/// Standard mDNS UDP port: 5353.
pub const MDNS_PORT: u16 = 5353;

/// Checks if a received UDP packet contains an mDNS query for "khvylyna.local".
pub fn is_query_for_hostname(packet: &[u8]) -> bool {
    // DNS header is 12 bytes; question follows
    if packet.len() < 12 {
        return false;
    }

    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    // Check if QR bit (bit 15) is 0 (query)
    if flags & 0x8000 != 0 {
        return false;
    }

    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    if qdcount == 0 {
        return false;
    }

    // Search for label sequence: 8, "khvylyna", 5, "local", 0
    let target = b"\x08khvylyna\x05local\x00";
    packet[12..].windows(target.len()).any(|w| w == target)
}

/// Builds an mDNS A-record response packet for "khvylyna.local" pointing to local IPv4.
pub fn build_a_response(ip: Ipv4Address, buffer: &mut [u8]) -> Option<usize> {
    if buffer.len() < 47 {
        return None;
    }

    let mut cursor = 0;

    // Transaction ID (0 for mDNS responses)
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x00]);
    cursor += 2;

    // Flags: Response (0x8400 = QR=1, AA=1, Opcode=0, RA=0, RCODE=0)
    buffer[cursor..cursor + 2].copy_from_slice(&[0x84, 0x00]);
    cursor += 2;

    // Questions: 0
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x00]);
    cursor += 2;

    // Answer RRs: 1
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x01]);
    cursor += 2;

    // Authority RRs: 0
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x00]);
    cursor += 2;

    // Additional RRs: 0
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x00]);
    cursor += 2;

    // Answer Name: \x08khvylyna\x05local\x00
    let name = b"\x08khvylyna\x05local\x00";
    buffer[cursor..cursor + name.len()].copy_from_slice(name);
    cursor += name.len();

    // Type: A (0x0001)
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x01]);
    cursor += 2;

    // Class: IN (0x0001) + Flush cache bit (0x8000) = 0x8001
    buffer[cursor..cursor + 2].copy_from_slice(&[0x80, 0x01]);
    cursor += 2;

    // TTL: 120 seconds (0x00000078)
    buffer[cursor..cursor + 4].copy_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    cursor += 4;

    // Data Length: 4 bytes (IPv4)
    buffer[cursor..cursor + 2].copy_from_slice(&[0x00, 0x04]);
    cursor += 2;

    // IPv4 Address data
    buffer[cursor..cursor + 4].copy_from_slice(&ip.octets());
    cursor += 4;

    Some(cursor)
}
