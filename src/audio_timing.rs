//! Timing rules for the ceremony's audio blocks.

use core::time::Duration;
use khvylyna_core::PresetBlock;

pub(crate) fn pause_after(blocks: &[PresetBlock], index: usize) -> Duration {
    if matches!(
        (blocks.get(index), blocks.get(index.saturating_add(1))),
        (
            Some(PresetBlock::Announcement | PresetBlock::Bell),
            Some(PresetBlock::Metronome)
        )
    ) {
        khvylyna_core::scheduler::lead_in_pause(blocks)
    } else {
        Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use khvylyna_core::PresetBlock::{Announcement, Bell, Metronome, Silence};
    const METRONOME_LEAD_PAUSE: Duration = khvylyna_core::scheduler::METRONOME_LEAD_PAUSE;

    #[test]
    fn pauses_only_after_the_opening_block_before_metronome() {
        let blocks = [Bell, Announcement, Metronome];
        assert_eq!(pause_after(&blocks, 0), Duration::ZERO);
        assert_eq!(pause_after(&blocks, 1), METRONOME_LEAD_PAUSE);
        assert_eq!(pause_after(&blocks, 2), Duration::ZERO);
        assert_eq!(pause_after(&blocks, usize::MAX), Duration::ZERO);
    }

    #[test]
    fn does_not_add_a_pause_to_silence_or_a_metronome_without_lead_in() {
        assert_eq!(pause_after(&[Announcement, Silence], 0), Duration::ZERO);
        assert_eq!(pause_after(&[Metronome], 0), Duration::ZERO);
        assert_eq!(pause_after(&[Silence, Bell, Metronome], 1), Duration::ZERO);
        assert_eq!(pause_after(&[], 0), Duration::ZERO);
    }

    #[test]
    fn pauses_after_an_opening_bell_or_announcement() {
        assert_eq!(pause_after(&[Bell, Metronome], 0), METRONOME_LEAD_PAUSE);
        assert_eq!(
            pause_after(&[Announcement, Metronome], 0),
            METRONOME_LEAD_PAUSE
        );
    }
}
