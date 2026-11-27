//! Multicast DNS (mDNS) and DNS-SD responder for device discovery on local networks.

use embassy_net::Ipv4Address;

/// Default hostname advertised over mDNS (resolves to khvylyna.local).
pub const MDNS_HOSTNAME: &str = "khvylyna";

/// DNS-SD service type advertised by Minute of Silence devices.
pub const SERVICE_TYPE: &str = "_khvylyna._tcp.local";

/// Multicast DNS standard IPv4 destination address: 224.0.0.251.
pub const MDNS_MCAST_IPV4: Ipv4Address = Ipv4Address::new(224, 0, 0, 251);

/// Standard mDNS UDP port: 5353.
pub const MDNS_PORT: u16 = 5353;

/// Pairing server port advertised in the DNS-SD SRV record.
pub const HTTP_PORT: u16 = crate::http_server::PAIRING_PORT;

const HOSTNAME_WIRE: &[u8] = b"\x08khvylyna\x05local\x00";
const SERVICE_WIRE: &[u8] = b"\x09_khvylyna\x04_tcp\x05local\x00";
const INSTANCE_WIRE: &[u8] = b"\x08khvylyna\x09_khvylyna\x04_tcp\x05local\x00";
const TXT_RDATA: &[u8] = b"\x0dkind=hardware\x13name=Khvylyna ESP32\x09ver=0.1.0\x07proto=1";

/// Checks if a received UDP packet contains an mDNS query for `khvylyna.local`.
pub fn is_query_for_hostname(packet: &[u8]) -> bool {
    is_query_matching(packet, HOSTNAME_WIRE)
}

/// Checks if a received UDP packet contains a DNS-SD query for `_khvylyna._tcp.local`.
pub fn is_query_for_service(packet: &[u8]) -> bool {
    is_query_matching(packet, SERVICE_WIRE)
}

fn is_query_matching(packet: &[u8], target: &[u8]) -> bool {
    if packet.len() < 12 {
        return false;
    }

    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    // QR bit (bit 15) must be 0 for a query
    if flags & 0x8000 != 0 {
        return false;
    }

    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    if qdcount == 0 {
        return false;
    }

    packet[12..]
        .windows(target.len())
        .any(|w| w.eq_ignore_ascii_case(target))
}

/// Builds an mDNS A-record response packet for `khvylyna.local` pointing to `ip`.
pub fn build_a_response(ip: Ipv4Address, buffer: &mut [u8]) -> Option<usize> {
    let mut cursor = 0;
    write_slice(
        buffer,
        &mut cursor,
        &[
            0x00, 0x00, // Transaction ID
            0x84, 0x00, // Flags: QR=1, AA=1
            0x00, 0x00, // QDCOUNT=0
            0x00, 0x01, // ANCOUNT=1
            0x00, 0x00, // NSCOUNT=0
            0x00, 0x00, // ARCOUNT=0
        ],
    )?;

    write_rr(buffer, &mut cursor, HOSTNAME_WIRE, 1, 0x8001, &ip.octets())?;
    Some(cursor)
}

/// Builds a DNS-SD mDNS response packet advertising `_khvylyna._tcp.local`
/// with PTR, SRV (port 80 -> `khvylyna.local`), TXT (`kind=hardware`, `proto=1`), and A records.
pub fn build_dnssd_response(ip: Ipv4Address, buffer: &mut [u8]) -> Option<usize> {
    let mut cursor = 0;
    write_slice(
        buffer,
        &mut cursor,
        &[
            0x00, 0x00, // Transaction ID
            0x84, 0x00, // Flags: QR=1, AA=1
            0x00, 0x00, // QDCOUNT=0
            0x00, 0x01, // ANCOUNT=1 (PTR)
            0x00, 0x00, // NSCOUNT=0
            0x00, 0x03, // ARCOUNT=3 (SRV, TXT, A)
        ],
    )?;

    // PTR: _khvylyna._tcp.local -> khvylyna._khvylyna._tcp.local
    write_rr(buffer, &mut cursor, SERVICE_WIRE, 12, 0x0001, INSTANCE_WIRE)?;

    // SRV: khvylyna._khvylyna._tcp.local -> (0, 0, HTTP_PORT, khvylyna.local)
    let mut srv_rdata = [0u8; 6 + 16];
    let port_bytes = HTTP_PORT.to_be_bytes();
    srv_rdata[4] = port_bytes[0];
    srv_rdata[5] = port_bytes[1];
    srv_rdata[6..].copy_from_slice(HOSTNAME_WIRE);
    write_rr(buffer, &mut cursor, INSTANCE_WIRE, 33, 0x8001, &srv_rdata)?;

    // TXT: kind=hardware, name=Khvylyna ESP32, ver=0.1.0
    write_rr(buffer, &mut cursor, INSTANCE_WIRE, 16, 0x8001, TXT_RDATA)?;

    // A: khvylyna.local -> IPv4
    write_rr(buffer, &mut cursor, HOSTNAME_WIRE, 1, 0x8001, &ip.octets())?;

    Some(cursor)
}

fn write_rr(
    buffer: &mut [u8],
    cursor: &mut usize,
    name_wire: &[u8],
    rtype: u16,
    rclass: u16,
    rdata: &[u8],
) -> Option<()> {
    write_slice(buffer, cursor, name_wire)?;
    write_slice(buffer, cursor, &rtype.to_be_bytes())?;
    write_slice(buffer, cursor, &rclass.to_be_bytes())?;
    write_slice(buffer, cursor, &120u32.to_be_bytes())?;
    write_slice(buffer, cursor, &(rdata.len() as u16).to_be_bytes())?;
    write_slice(buffer, cursor, rdata)
}

fn write_slice(buffer: &mut [u8], cursor: &mut usize, data: &[u8]) -> Option<()> {
    let end = cursor.checked_add(data.len())?;
    if end > buffer.len() {
        return None;
    }
    buffer[*cursor..end].copy_from_slice(data);
    *cursor = end;
    Some(())
}
