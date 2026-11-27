use khvylyna_pairing::{
    pake::{receiver_response, SenderExchange},
    protocol,
};
use rand_core::{CryptoRng, RngCore};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vectors {
    version: u8,
    suite: u8,
    code: String,
    max_payload: u16,
    sender_seed: u8,
    receiver_seed: u8,
    confirm_seed: u8,
    data_seed: u8,
    ack_seed: u8,
    plaintext: String,
    pake_init: String,
    pake_response: String,
    confirm: String,
    data: String,
    ack: String,
}

struct FixedRng {
    seed: u8,
    cursor: u8,
}

impl FixedRng {
    fn new(seed: u8) -> Self {
        Self { seed, cursor: 0 }
    }

    fn next_byte(&mut self) -> u8 {
        let byte = self.seed.wrapping_add(self.cursor);
        self.cursor = self.cursor.wrapping_add(1);
        byte
    }
}

impl RngCore for FixedRng {
    fn next_u32(&mut self) -> u32 {
        u32::from_le_bytes([
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
        ])
    }

    fn next_u64(&mut self) -> u64 {
        u64::from_le_bytes([
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
        ])
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for byte in dest {
            *byte = self.next_byte();
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for FixedRng {}

fn main() {}

#[cfg(test)]
#[path = "../../../src/transfer.rs"]
mod transfer;

#[cfg(test)]
#[path = "../../../src/led_pattern.rs"]
mod led_pattern;

#[cfg(test)]
#[path = "../../../src/pcm.rs"]
mod pcm;

#[cfg(test)]
#[path = "../../../src/wav.rs"]
mod wav;

#[cfg(test)]
#[path = "../../../src/flash_layout.rs"]
mod flash_layout;

#[cfg(test)]
#[path = "../../../src/audio_timing.rs"]
mod audio_timing;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_imports_the_enclosed_settings() {
        let mut settings = khvylyna_core::Settings::default();
        settings.volume = 37;
        let bundle = khvylyna_core::ExportBundle {
            settings,
            personal_dates: Vec::new(),
        };
        let payload = serde_json::json!({
            "sender_name": "Phone",
            "json": serde_json::to_string(&bundle).expect("bundle"),
        });
        let bytes = serde_json::to_vec(&payload).expect("payload");
        assert_eq!(
            transfer::parse_settings(&bytes).expect("settings").volume,
            37
        );
    }

    #[test]
    fn transfer_rejects_missing_malformed_and_invalid_settings() {
        for payload in [
            r#"{}"#,
            r#"{"sender_name":"Phone","json":"{}"}"#,
            r#"{"sender_name":"Phone","json":"not json"}"#,
            r#"{"sender_name":"Phone","json":{"settings":{}}}"#,
            r#"{"sender_name":"Phone","json":"{\"settings\":{\"volume\":101},\"personalDates\":[]}"}"#,
        ] {
            assert!(
                transfer::parse_settings(payload.as_bytes()).is_none(),
                "{payload}"
            );
        }
    }

    #[test]
    fn device_matches_shared_pairing_vectors() {
        let vectors: Vectors =
            serde_json::from_str(include_str!("../pairing_vectors.json")).expect("valid vectors");
        assert_eq!(vectors.version, protocol::VERSION);
        assert_eq!(vectors.suite, protocol::SUITE_CPACE_RISTRETTO_AES256_GCM);

        let (sender, init) = SenderExchange::start(
            &vectors.code,
            vectors.max_payload,
            &mut FixedRng::new(vectors.sender_seed),
        )
        .expect("sender");
        assert_eq!(hex::encode(&init), vectors.pake_init);

        let (attempt, response) = receiver_response(
            &vectors.code,
            &hex::decode(&vectors.pake_init).expect("init hex"),
            vectors.max_payload,
            &mut FixedRng::new(vectors.receiver_seed),
        )
        .expect("receiver");
        assert_eq!(hex::encode(&response), vectors.pake_response);

        let sender_secret = sender
            .finish(&vectors.code, &response)
            .expect("sender finish");
        let confirm = sender_secret
            .seal_confirm(&mut FixedRng::new(vectors.confirm_seed))
            .expect("confirm");
        assert_eq!(hex::encode(&confirm), vectors.confirm);
        let confirmed = attempt.verify_confirm(&confirm).expect("verify confirm");
        let receiver_secret = confirmed.into_secret();

        let plaintext = hex::decode(&vectors.plaintext).expect("plaintext hex");
        let data = sender_secret
            .seal_data(&plaintext, &mut FixedRng::new(vectors.data_seed))
            .expect("data");
        assert_eq!(hex::encode(&data), vectors.data);
        assert_eq!(
            receiver_secret
                .open_data(&data)
                .expect("open data")
                .as_slice(),
            plaintext
        );

        let ack = receiver_secret
            .seal_ack(true, &mut FixedRng::new(vectors.ack_seed))
            .expect("ack");
        assert_eq!(hex::encode(&ack), vectors.ack);
        assert!(sender_secret.open_ack(&ack).expect("open ack"));
    }
}
