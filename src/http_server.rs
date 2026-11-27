use alloc::{vec, vec::Vec};
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use critical_section::Mutex;
use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Instant, Timer};
use esp_println::println;
use khvylyna_core::Settings;
use khvylyna_pairing::{
    ProtocolError,
    protocol::{self, FrameKind},
    window::PairingWindow,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

pub const PAIRING_PORT: u16 = 80;

const NO_CODE: u32 = u32::MAX;
const PAIRING_REQUEST_DEBOUNCE_MS: u32 = 500;

static PENDING_SETTINGS: Mutex<RefCell<Option<Settings>>> = Mutex::new(RefCell::new(None));
static SETTINGS_UPDATED: AtomicBool = AtomicBool::new(false);
static PAIRING_REQUESTED: AtomicBool = AtomicBool::new(false);
static PAIRING_CODE: AtomicU32 = AtomicU32::new(NO_CODE);
static LAST_PAIRING_REQUEST_MS: AtomicU32 = AtomicU32::new(0);

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

pub fn request_pairing() {
    let now = now_ms() as u32;
    let previous = LAST_PAIRING_REQUEST_MS.swap(now, Ordering::AcqRel);
    if now.saturating_sub(previous) >= PAIRING_REQUEST_DEBOUNCE_MS {
        PAIRING_REQUESTED.store(true, Ordering::Release);
    }
}

pub fn take_pairing_request() -> bool {
    PAIRING_REQUESTED.swap(false, Ordering::AcqRel)
}

pub fn pairing_code() -> Option<u32> {
    let code = PAIRING_CODE.load(Ordering::Acquire);
    (code != NO_CODE).then_some(code)
}

fn set_pairing_code(code: &str) {
    let packed = code
        .as_bytes()
        .iter()
        .try_fold(0u32, |value, byte| {
            if byte.is_ascii_digit() {
                Some(value * 10 + u32::from(*byte - b'0'))
            } else {
                None
            }
        })
        .unwrap_or(NO_CODE);
    PAIRING_CODE.store(packed, Ordering::Release);
}

fn clear_pairing_code() {
    PAIRING_CODE.store(NO_CODE, Ordering::Release);
}

fn now_ms() -> u64 {
    Instant::now().as_millis() as u64
}

fn error_code(error: &ProtocolError) -> u8 {
    match error {
        ProtocolError::UnsupportedVersion => protocol::ERROR_UNSUPPORTED_VERSION,
        ProtocolError::UnsupportedSuite => protocol::ERROR_UNSUPPORTED_SUITE,
        ProtocolError::WindowUnavailable => protocol::ERROR_WINDOW_UNAVAILABLE,
        ProtocolError::PayloadTooLarge => protocol::ERROR_PAYLOAD_TOO_LARGE,
        _ => protocol::ERROR_INVALID_MESSAGE,
    }
}

async fn write_all(socket: &mut TcpSocket<'_>, bytes: &[u8]) -> Result<(), ()> {
    let mut written = 0;
    while written < bytes.len() {
        match socket.write(&bytes[written..]).await {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => written += n,
        }
    }
    socket.flush().await.map_err(|_| ())
}

async fn read_exact(socket: &mut TcpSocket<'_>, bytes: &mut [u8]) -> Result<(), ()> {
    let mut read = 0;
    while read < bytes.len() {
        match socket.read(&mut bytes[read..]).await {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => read += n,
        }
    }
    Ok(())
}

async fn write_frame(
    socket: &mut TcpSocket<'_>,
    kind: FrameKind,
    payload: &[u8],
) -> Result<(), ()> {
    let mut header = [0u8; protocol::HEADER_LEN];
    protocol::write_header(kind, payload.len(), &mut header).map_err(|_| ())?;
    write_all(socket, &header).await?;
    write_all(socket, payload).await
}

async fn read_frame(
    socket: &mut TcpSocket<'_>,
    negotiated_max: u16,
) -> Result<(protocol::FrameHeader, Vec<u8>), ProtocolError> {
    let mut header = [0u8; protocol::HEADER_LEN];
    read_exact(socket, &mut header)
        .await
        .map_err(|()| ProtocolError::InvalidFrame)?;
    let parsed = protocol::parse_header(&header, negotiated_max)?;
    let mut payload = vec![0u8; parsed.len];
    read_exact(socket, &mut payload)
        .await
        .map_err(|()| ProtocolError::InvalidFrame)?;
    Ok((parsed, payload))
}

async fn read_expected(
    socket: &mut TcpSocket<'_>,
    kind: FrameKind,
    negotiated_max: u16,
) -> Result<Vec<u8>, ProtocolError> {
    let (header, payload) = read_frame(socket, negotiated_max).await?;
    if header.kind != kind {
        return Err(ProtocolError::InvalidFrame);
    }
    Ok(payload)
}

async fn reject(socket: &mut TcpSocket<'_>, error: ProtocolError) {
    let _ = write_frame(socket, FrameKind::ErrorFrame, &[error_code(&error)]).await;
}

fn negotiated_limit(hello: &[u8]) -> Result<u16, ProtocolError> {
    let offer: [u8; 2] = hello.try_into().map_err(|_| ProtocolError::InvalidFrame)?;
    let offer = u16::from_be_bytes(offer);
    if offer == 0 {
        return Err(ProtocolError::PayloadTooLarge);
    }
    Ok(offer.min(protocol::DEVICE_MAX_PAYLOAD))
}

fn apply_settings(bytes: &[u8]) -> bool {
    let Some(settings) = crate::transfer::parse_settings(bytes) else {
        println!("[Pairing] Invalid settings transfer");
        return false;
    };
    println!(
        "[Pairing] Applied settings (voice: {:?}, volume: {})",
        settings.announcement_voice, settings.volume
    );
    store_updated_settings(settings);
    true
}

async fn serve_peer(socket: &mut TcpSocket<'_>, window: &mut PairingWindow, rng: &mut ChaCha20Rng) {
    let hello = match read_frame(socket, 0).await {
        Ok((header, payload)) if header.kind == FrameKind::Hello => payload,
        Ok(_) => {
            reject(socket, ProtocolError::InvalidFrame).await;
            return;
        }
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let limit = match negotiated_limit(&hello) {
        Ok(limit) => limit,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    if write_frame(socket, FrameKind::HelloAck, &limit.to_be_bytes())
        .await
        .is_err()
    {
        return;
    }

    let request = match read_expected(socket, FrameKind::PakeInit, limit).await {
        Ok(request) => request,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let (attempt, response) = match window.start(&request, now_ms(), limit, rng) {
        Ok(result) => result,
        Err(error) => {
            clear_pairing_code();
            reject(socket, error).await;
            return;
        }
    };
    if write_frame(socket, FrameKind::PakeResponse, &response)
        .await
        .is_err()
    {
        return;
    }

    let confirm = match read_expected(socket, FrameKind::Confirm, limit).await {
        Ok(confirm) => confirm,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let confirmed = match attempt.verify_confirm(&confirm) {
        Ok(confirmed) => confirmed,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let secret = match window.finish(confirmed, now_ms()) {
        Ok(secret) => secret,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    clear_pairing_code();

    let data = match read_expected(socket, FrameKind::Data, limit).await {
        Ok(data) => data,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let payload = match secret.open_data(&data) {
        Ok(payload) => payload,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let accepted = apply_settings(&payload);
    let ack = match secret.seal_ack(accepted, rng) {
        Ok(ack) => ack,
        Err(error) => {
            reject(socket, error).await;
            return;
        }
    };
    let _ = write_frame(socket, FrameKind::Ack, &ack).await;
}

#[embassy_executor::task]
pub async fn pairing_task(stack: Stack<'static>, seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut window: Option<PairingWindow> = None;
    let mut rx_buf = [0u8; 2048];
    let mut tx_buf = [0u8; 512];

    println!(
        "[Pairing] Waiting for a long button press on port {}",
        PAIRING_PORT
    );

    loop {
        if take_pairing_request() {
            match PairingWindow::new(now_ms(), &mut rng) {
                Ok(fresh) => {
                    if let Some(code) = fresh.code(now_ms()) {
                        set_pairing_code(code);
                        println!("[Pairing] Code: {}", code);
                        window = Some(fresh);
                    } else {
                        clear_pairing_code();
                        window = None;
                        println!("[Pairing] Code expired before display");
                    }
                }
                Err(error) => println!("[Pairing] Failed to create window: {}", error),
            }
        }

        let Some(active) = window.as_mut() else {
            Timer::after(Duration::from_millis(100)).await;
            continue;
        };
        if active.code(now_ms()).is_none() {
            clear_pairing_code();
            window = None;
            println!("[Pairing] Code expired");
            continue;
        }

        let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
        socket.set_timeout(Some(Duration::from_secs(20)));
        match socket.accept(PAIRING_PORT).await {
            Ok(()) => {
                serve_peer(&mut socket, active, &mut rng).await;
                let _ = socket.flush().await;
            }
            Err(error) => {
                println!("[Pairing] Accept error: {:?}", error);
                Timer::after(Duration::from_millis(100)).await;
            }
        }
        socket.close();
    }
}
