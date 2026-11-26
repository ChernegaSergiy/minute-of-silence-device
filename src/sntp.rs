use core::net::{IpAddr, SocketAddr};
use embassy_net::Stack;
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use esp_hal::rtc_cntl::Rtc;
use esp_println::println;
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;

const USEC_IN_SEC: u64 = 1_000_000;

#[derive(Clone, Copy)]
struct Timestamp<'a> {
    rtc: &'a Rtc<'a>,
    current_time_us: u64,
}

impl NtpTimestampGenerator for Timestamp<'_> {
    fn init(&mut self) {
        self.current_time_us = self.rtc.current_time_us();
    }

    fn timestamp_sec(&self) -> u64 {
        self.current_time_us / USEC_IN_SEC
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.current_time_us % USEC_IN_SEC) as u32
    }
}

pub async fn synchronize_time(stack: &Stack<'_>, rtc: &Rtc<'_>, ntp_server: &str) -> bool {
    println!("Synchronizing time via SNTP from: {}", ntp_server);
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buffer = [0u8; 512];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buffer = [0u8; 512];

    let Ok(addrs) = stack.dns_query(ntp_server, DnsQueryType::A).await else {
        println!("Failed to resolve NTP server DNS");
        return false;
    };

    let Some(&first_addr) = addrs.first() else {
        println!("DNS returned empty address list for NTP");
        return false;
    };

    let addr: IpAddr = first_addr.into();
    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    if socket.bind(123).is_err() {
        println!("Failed to bind UDP port 123 for SNTP");
        return false;
    }

    let socket_wrapper = UdpSocketWrapper::new(socket);
    let context = NtpContext::new(Timestamp {
        rtc,
        current_time_us: 0,
    });

    match get_time(SocketAddr::from((addr, 123)), &socket_wrapper, context).await {
        Ok(time) => {
            let epoch_us = (time.sec() as u64 * USEC_IN_SEC)
                + ((time.sec_fraction() as u64 * USEC_IN_SEC) >> 32);
            rtc.set_current_time_us(epoch_us);
            println!("NTP synchronized! Epoch: {} s", time.sec());
            true
        }
        Err(_) => {
            println!("Failed to receive NTP response");
            false
        }
    }
}
