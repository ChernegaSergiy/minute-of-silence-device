use embassy_time::{Duration, with_timeout};
use esp_hal::Async;
use esp_hal::dma::DmaTxStreamBuf;
use esp_hal::i2s::master::{I2sTx, I2sTxDmaTransfer};
use esp_println::println;

use crate::audio::PcmSink;
use crate::pcm::StereoPcm;
use crate::wav::WavHeader;

const SAMPLE_RATE: u32 = 44_100;
const DESCRIPTOR_BYTES: usize = 512;
const PREFILL_BYTES: usize = 4096;

pub(crate) struct I2sSink<'a, 'd> {
    transmitter: &'a mut Option<I2sTx<'d, Async>>,
    buffer: &'a mut Option<DmaTxStreamBuf>,
    transfer: Option<I2sTxDmaTransfer<'d, Async, DmaTxStreamBuf>>,
    converter: Option<StereoPcm>,
    header: Option<WavHeader>,
    volume: u8,
    pending: [u8; PREFILL_BYTES],
    pending_len: usize,
    started: bool,
}

impl<'a, 'd> I2sSink<'a, 'd> {
    pub(crate) fn new(
        transmitter: &'a mut Option<I2sTx<'d, Async>>,
        buffer: &'a mut Option<DmaTxStreamBuf>,
        volume: u8,
    ) -> Self {
        Self {
            transmitter,
            buffer,
            transfer: None,
            converter: None,
            header: None,
            volume,
            pending: [0; PREFILL_BYTES],
            pending_len: 0,
            started: false,
        }
    }

    fn configure(&mut self, header: &WavHeader) -> Result<(), ()> {
        if header.sample_rate != SAMPLE_RATE || header.bits_per_sample != 16 {
            println!("[Audio] I2S requires 16-bit PCM at 44100 Hz");
            return Err(());
        }
        self.converter = Some(StereoPcm::new(header.channels, self.volume).ok_or(())?);
        self.header = Some(*header);
        Ok(())
    }

    fn start_dma(&mut self) -> Result<(), ()> {
        let Some(transmitter) = self.transmitter.take() else {
            return Err(());
        };
        let Some(mut buffer) = self.buffer.take() else {
            *self.transmitter = Some(transmitter);
            return Err(());
        };
        if buffer.push(&self.pending[..self.pending_len]) != self.pending_len {
            *self.transmitter = Some(transmitter);
            self.restore_buffer(buffer);
            return Err(());
        }
        match transmitter.write(buffer) {
            Ok(transfer) => {
                self.transfer = Some(transfer);
                self.pending_len = 0;
                self.started = true;
                Ok(())
            }
            Err((_error, transmitter, buffer)) => {
                *self.transmitter = Some(transmitter);
                self.restore_buffer(buffer);
                Err(())
            }
        }
    }

    fn restore_buffer(&mut self, buffer: DmaTxStreamBuf) {
        let (descriptors, bytes) = buffer.split();
        *self.buffer = DmaTxStreamBuf::new(descriptors, bytes).ok();
    }

    async fn flush(&mut self) -> Result<(), ()> {
        let transfer = self.transfer.as_mut().ok_or(())?;
        let mut written = 0;
        while written < self.pending_len {
            if transfer.is_done() {
                return Err(());
            }
            let count = transfer.push(&self.pending[written..self.pending_len]);
            written += count;
            if count == 0 {
                with_timeout(Duration::from_secs(1), transfer.wait_for_available_async())
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
            }
        }
        self.pending_len = 0;
        Ok(())
    }

    pub(crate) async fn finish(mut self, success: bool) -> Result<(), bool> {
        let complete = self.converter.as_ref().is_some_and(StereoPcm::is_complete);
        let mut valid = success && complete;
        if valid && self.pending_len != 0 {
            let mut padded = self.pending_len.div_ceil(DESCRIPTOR_BYTES) * DESCRIPTOR_BYTES;
            if self.transfer.is_none() {
                padded = padded.max(2 * DESCRIPTOR_BYTES);
            }
            self.pending[self.pending_len..padded].fill(0);
            self.pending_len = padded;
            valid = if self.transfer.is_some() {
                self.flush().await.is_ok()
            } else {
                self.start_dma().is_ok()
            };
        }
        if valid {
            valid = match self.transfer.as_mut() {
                Some(transfer) => matches!(
                    with_timeout(Duration::from_secs(2), transfer.wait_for_done_async()).await,
                    Ok(Ok(()))
                ),
                None => false,
            };
        }
        let Some(transfer) = self.transfer.take() else {
            return Err(self.started);
        };
        let (transmitter, buffer) = transfer.stop();
        *self.transmitter = Some(transmitter);
        self.restore_buffer(buffer);
        if valid { Ok(()) } else { Err(self.started) }
    }
}

impl Drop for I2sSink<'_, '_> {
    fn drop(&mut self) {
        if let Some(transfer) = self.transfer.take() {
            let (transmitter, buffer) = transfer.stop();
            *self.transmitter = Some(transmitter);
            self.restore_buffer(buffer);
        }
    }
}

impl PcmSink for I2sSink<'_, '_> {
    async fn write(&mut self, chunk: &[u8], header: &WavHeader) -> Result<(), ()> {
        if self.header.is_none() {
            self.configure(header)?;
        } else if self.header.as_ref() != Some(header) {
            return Err(());
        }
        for &byte in chunk {
            let frame = self.converter.as_mut().ok_or(())?.push(byte);
            if let Some(frame) = frame {
                self.pending[self.pending_len..self.pending_len + frame.len()]
                    .copy_from_slice(&frame);
                self.pending_len += frame.len();
                if self.pending_len == PREFILL_BYTES {
                    if self.transfer.is_some() {
                        self.flush().await?;
                    } else {
                        self.start_dma()?;
                    }
                }
            }
        }
        Ok(())
    }
}
