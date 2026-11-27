//! Lightweight HTTP/1.1 server on port 80 for receiving `POST /api/bundle` settings updates.

use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use critical_section::Mutex;
use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer};
use esp_println::println;
use khvylyna_core::{ExportBundle, Settings};

/// HTTP server TCP port.
pub const HTTP_PORT: u16 = 80;

/// HTTP path for receiving serialized `ExportBundle` JSON payloads.
const BUNDLE_PATH: &str = "/api/bundle";

/// Maximum total HTTP request size accepted (16 KiB) to stay safe on ESP32 heap.
const MAX_REQUEST_BYTES: usize = 16 * 1024;

static PENDING_SETTINGS: Mutex<RefCell<Option<Settings>>> = Mutex::new(RefCell::new(None));
static SETTINGS_UPDATED: AtomicBool = AtomicBool::new(false);

/// Take newly received settings if the HTTP server accepted a valid bundle since the last check.
pub fn take_updated_settings() -> Option<Settings> {
    if !SETTINGS_UPDATED.swap(false, Ordering::AcqRel) {
        return None;
    }
    critical_section::with(|cs| PENDING_SETTINGS.borrow(cs).borrow_mut().take())
}

fn store_updated_settings(settings: Settings) {
    critical_section::with(|cs| {
        *PENDING_SETTINGS.borrow(cs).borrow_mut() = Some(settings);
    });
    SETTINGS_UPDATED.store(true, Ordering::Release);
}

#[embassy_executor::task]
pub async fn http_server_task(stack: Stack<'static>) {
    let mut rx_buf = [0u8; 2048];
    let mut tx_buf = [0u8; 512];

    println!(
        "[HTTP] Listening on port {} for POST {}",
        HTTP_PORT, BUNDLE_PATH
    );

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
        socket.set_timeout(Some(Duration::from_secs(5)));

        if let Err(err) = socket.accept(HTTP_PORT).await {
            println!("[HTTP] Accept error: {:?}", err);
            Timer::after(Duration::from_millis(100)).await;
            continue;
        }

        let status = handle_request(&mut socket).await;
        let _ = write_status(&mut socket, status).await;
        let _ = socket.flush().await;
        socket.close();
    }
}

async fn handle_request(socket: &mut TcpSocket<'_>) -> u16 {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 512];

    let header_end = loop {
        match socket.read(&mut chunk).await {
            Ok(0) => return 400,
            Ok(n) => {
                if buf.len().saturating_add(n) > MAX_REQUEST_BYTES {
                    return 413;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = find_header_end(&buf) {
                    break pos;
                }
            }
            Err(_) => return 400,
        }
    };

    let Ok(header_str) = core::str::from_utf8(&buf[..header_end]) else {
        return 400;
    };
    let mut lines = header_str.split("\r\n");
    let Some(req_line) = lines.next() else {
        return 400;
    };
    let mut parts = req_line.split_whitespace();
    let Some(method) = parts.next() else {
        return 400;
    };
    let Some(path) = parts.next() else {
        return 400;
    };

    if path != BUNDLE_PATH {
        return 404;
    }
    if method != "POST" {
        return 405;
    }

    let mut content_length: usize = 0;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                let Ok(len) = v.trim().parse::<usize>() else {
                    return 400;
                };
                content_length = len;
            }
        }
    }

    if content_length == 0 {
        return 400;
    }
    let body_start = header_end + 4;
    if body_start.saturating_add(content_length) > MAX_REQUEST_BYTES {
        return 413;
    }

    while buf.len().saturating_sub(body_start) < content_length {
        match socket.read(&mut chunk).await {
            Ok(0) => return 400,
            Ok(n) => {
                if buf.len().saturating_add(n) > MAX_REQUEST_BYTES {
                    return 413;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(_) => return 400,
        }
    }

    let Ok(body) = core::str::from_utf8(&buf[body_start..body_start + content_length]) else {
        return 400;
    };

    let mut settings = match serde_json::from_str::<ExportBundle>(body) {
        Ok(bundle) => bundle.settings,
        Err(_) => match serde_json::from_str::<Settings>(body) {
            Ok(direct) => direct,
            Err(_) => {
                println!("[HTTP] Invalid settings JSON payload");
                return 400;
            }
        },
    };

    settings.migrate();
    if settings.validate().is_err() {
        println!("[HTTP] Settings validation failed");
        return 400;
    }

    println!(
        "[HTTP] Applied incoming settings (voice: {:?}, volume: {})",
        settings.announcement_voice, settings.volume
    );
    store_updated_settings(settings);
    200
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn write_status(socket: &mut TcpSocket<'_>, code: u16) -> Result<(), ()> {
    let resp: &[u8] = match code {
        200 => b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        400 => b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        404 => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        405 => b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        413 => b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        _ => {
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        }
    };
    let mut written = 0;
    while written < resp.len() {
        match socket.write(&resp[written..]).await {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => written += n,
        }
    }
    Ok(())
}
