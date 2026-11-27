const PULSE_MS: u64 = 300;
const ZERO_MS: u64 = 1200;
const DIGIT_PAUSE_MS: u64 = 1200;
const CYCLE_PAUSE_MS: u64 = 3000;

#[derive(Default)]
pub struct CodeDisplay {
    code: Option<u32>,
    started_at_ms: u64,
}

impl CodeDisplay {
    pub fn is_on(&mut self, code: Option<u32>, now_ms: u64) -> bool {
        if self.code != code {
            self.code = code;
            self.started_at_ms = now_ms;
        }
        let Some(mut value) = code else {
            return true;
        };

        let mut digits = [0u8; 6];
        for digit in digits.iter_mut().rev() {
            *digit = (value % 10) as u8;
            value /= 10;
        }

        let cycle_ms = CYCLE_PAUSE_MS
            + digits
                .iter()
                .map(|digit| digit_duration(*digit) + DIGIT_PAUSE_MS)
                .sum::<u64>();
        let mut offset = now_ms.saturating_sub(self.started_at_ms) % cycle_ms;
        if offset < CYCLE_PAUSE_MS {
            return false;
        }
        offset -= CYCLE_PAUSE_MS;
        for digit in digits {
            let duration = digit_duration(digit);
            if offset < duration {
                return digit == 0 || offset % (2 * PULSE_MS) < PULSE_MS;
            }
            offset -= duration;
            if offset < DIGIT_PAUSE_MS {
                return false;
            }
            offset -= DIGIT_PAUSE_MS;
        }
        false
    }
}

fn digit_duration(digit: u8) -> u64 {
    if digit == 0 {
        ZERO_MS
    } else {
        u64::from(digit) * 2 * PULSE_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_leading_zero_and_separates_digits() {
        let mut display = CodeDisplay::default();
        assert!(!display.is_on(Some(12345), 0));
        assert!(display.is_on(Some(12345), 3000));
        assert!(display.is_on(Some(12345), 4199));
        assert!(!display.is_on(Some(12345), 4200));
        assert!(display.is_on(Some(12345), 5400));
        assert!(!display.is_on(Some(12345), 5700));
    }

    #[test]
    fn repeats_the_code_and_resets_on_change() {
        let mut display = CodeDisplay::default();
        let cycle_ms = CYCLE_PAUSE_MS + 6 * (ZERO_MS + DIGIT_PAUSE_MS);
        assert!(!display.is_on(Some(0), 100));
        assert!(display.is_on(Some(0), 3100));
        assert!(!display.is_on(Some(0), 100 + cycle_ms));
        assert!(!display.is_on(Some(123456), 100 + cycle_ms));
        assert!(display.is_on(None, 100 + cycle_ms));
    }
}
