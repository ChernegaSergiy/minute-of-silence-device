//! Power-loss-safe record format for settings stored in two alternating flash slots.
//!
//! A record is written into the slot that does not hold the newest valid record, so the previous
//! configuration stays intact until the new one is completely written. A torn write leaves a
//! record whose checksum does not match, and it is ignored in favour of the other slot.

use alloc::vec::Vec;

/// Number of alternating slots.
pub const SLOT_COUNT: usize = 2;
/// Size reserved for one slot.
pub const SLOT_SIZE: u32 = 0x4000;
/// Size of the record header.
pub const HEADER_LEN: usize = 16;
/// Largest payload that fits into one slot.
pub const MAX_PAYLOAD: usize = SLOT_SIZE as usize - HEADER_LEN;
/// Flash write granularity; records are padded to a multiple of it.
pub const WRITE_ALIGN: usize = 4;

const MAGIC: [u8; 4] = *b"KSET";
const VERSION: u8 = 1;

/// A checksummed record found in a slot.
#[derive(Debug, PartialEq, Eq)]
pub struct Record<'a> {
    /// Monotonic (wrapping) sequence number; the newest record has the greatest one.
    pub sequence: u32,
    /// Stored payload.
    pub payload: &'a [u8],
}

/// Where and under which sequence number the next record must be written.
#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    /// Slot index to overwrite.
    pub slot: usize,
    /// Sequence number of the new record.
    pub sequence: u32,
}

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        for byte in *part {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
    }
    !crc
}

/// Returns the payload length announced by a header, if the header looks like a record.
pub fn announced_len(header: &[u8]) -> Option<usize> {
    if header.len() < HEADER_LEN || header[..4] != MAGIC || header[4] != VERSION {
        return None;
    }
    let len = usize::from(u16::from_le_bytes([header[6], header[7]]));
    (len <= MAX_PAYLOAD).then_some(len)
}

/// Total number of bytes that must be read to verify the record behind `header`.
pub fn record_len(header: &[u8]) -> Option<usize> {
    announced_len(header).map(|len| HEADER_LEN + len)
}

/// Builds the bytes to write into a slot, padded to the flash write granularity.
pub fn encode(sequence: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if payload.len() > MAX_PAYLOAD {
        return None;
    }
    let len = u16::try_from(payload.len()).ok()?.to_le_bytes();
    let sequence = sequence.to_le_bytes();
    let meta = [
        VERSION,
        0,
        len[0],
        len[1],
        sequence[0],
        sequence[1],
        sequence[2],
        sequence[3],
    ];
    let crc = crc32(&[&meta, payload]);
    let mut bytes = Vec::with_capacity((HEADER_LEN + payload.len()).next_multiple_of(WRITE_ALIGN));
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&meta);
    bytes.extend_from_slice(&crc.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes.resize(bytes.len().next_multiple_of(WRITE_ALIGN), 0xFF);
    Some(bytes)
}

/// Verifies the record at the start of `bytes`.
pub fn decode(bytes: &[u8]) -> Option<Record<'_>> {
    let len = announced_len(bytes)?;
    let payload = bytes.get(HEADER_LEN..HEADER_LEN + len)?;
    let stored = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if crc32(&[&bytes[4..12], payload]) != stored {
        return None;
    }
    Some(Record {
        sequence: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        payload,
    })
}

fn is_newer(candidate: u32, other: u32) -> bool {
    (candidate.wrapping_sub(other) as i32) > 0
}

/// Index of the slot holding the newest valid record.
pub fn newest(slots: [Option<u32>; SLOT_COUNT]) -> Option<usize> {
    match slots {
        [None, None] => None,
        [Some(_), None] => Some(0),
        [None, Some(_)] => Some(1),
        [Some(first), Some(second)] => Some(if is_newer(second, first) { 1 } else { 0 }),
    }
}

/// Chooses the slot for the next write: never the one holding the newest valid record.
pub fn next_target(slots: [Option<u32>; SLOT_COUNT]) -> Target {
    match newest(slots) {
        Some(current) => Target {
            slot: (current + 1) % SLOT_COUNT,
            sequence: slots[current].unwrap_or(0).wrapping_add(1),
        },
        None => Target {
            slot: 0,
            sequence: 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_pads_to_the_write_granularity() {
        let bytes = encode(7, b"abcde").expect("encode");
        assert_eq!(bytes.len() % WRITE_ALIGN, 0);
        assert_eq!(record_len(&bytes), Some(HEADER_LEN + 5));
        let record = decode(&bytes).expect("decode");
        assert_eq!(record.sequence, 7);
        assert_eq!(record.payload, b"abcde");
    }

    #[test]
    fn accepts_the_largest_payload_and_rejects_larger_ones() {
        assert!(encode(1, &alloc::vec![1; MAX_PAYLOAD]).is_some());
        assert!(encode(1, &alloc::vec![1; MAX_PAYLOAD + 1]).is_none());
    }

    #[test]
    fn erased_or_foreign_flash_is_not_a_record() {
        assert!(decode(&[0xFF; 64]).is_none());
        assert!(decode(&[0; 64]).is_none());
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn any_flipped_bit_invalidates_the_record() {
        let bytes = encode(3, b"settings").expect("encode");
        for index in 0..HEADER_LEN + 8 {
            for bit in 0..8 {
                let mut broken = bytes.clone();
                broken[index] ^= 1 << bit;
                assert!(decode(&broken).is_none(), "byte {index} bit {bit}");
            }
        }
    }

    #[test]
    fn a_truncated_write_is_rejected() {
        let bytes = encode(3, b"settings").expect("encode");
        for cut in 0..HEADER_LEN + 8 {
            assert!(decode(&bytes[..cut]).is_none(), "cut at {cut}");
        }
    }

    #[test]
    fn announced_length_beyond_the_slot_is_rejected() {
        let mut bytes = encode(1, b"x").expect("encode");
        let oversized = u16::try_from(MAX_PAYLOAD + 1).expect("fits").to_le_bytes();
        bytes[6] = oversized[0];
        bytes[7] = oversized[1];
        assert_eq!(announced_len(&bytes), None);
    }

    #[test]
    fn newest_valid_slot_wins() {
        assert_eq!(newest([None, None]), None);
        assert_eq!(newest([Some(4), None]), Some(0));
        assert_eq!(newest([None, Some(4)]), Some(1));
        assert_eq!(newest([Some(4), Some(5)]), Some(1));
        assert_eq!(newest([Some(6), Some(5)]), Some(0));
    }

    #[test]
    fn sequence_comparison_survives_wraparound() {
        assert_eq!(newest([Some(u32::MAX), Some(0)]), Some(1));
        assert_eq!(newest([Some(1), Some(u32::MAX)]), Some(0));
        let target = next_target([Some(u32::MAX), None]);
        assert_eq!(
            target,
            Target {
                slot: 1,
                sequence: 0
            }
        );
    }

    #[test]
    fn next_write_never_targets_the_newest_valid_slot() {
        assert_eq!(
            next_target([None, None]),
            Target {
                slot: 0,
                sequence: 1
            }
        );
        assert_eq!(
            next_target([Some(1), None]),
            Target {
                slot: 1,
                sequence: 2
            }
        );
        assert_eq!(
            next_target([Some(1), Some(2)]),
            Target {
                slot: 0,
                sequence: 3
            }
        );
        assert_eq!(
            next_target([Some(3), Some(2)]),
            Target {
                slot: 1,
                sequence: 4
            }
        );
        assert_eq!(
            next_target([None, Some(9)]),
            Target {
                slot: 0,
                sequence: 10
            }
        );
    }

    #[test]
    fn a_torn_write_keeps_the_previous_record_selected() {
        let old = encode(1, b"old").expect("encode");
        let new = encode(2, b"new settings").expect("encode");
        let torn = &new[..new.len() - 6];
        let slots = [
            decode(&old).map(|r| r.sequence),
            decode(torn).map(|r| r.sequence),
        ];
        assert_eq!(newest(slots), Some(0));
        assert_eq!(next_target(slots).slot, 1);
    }
}
