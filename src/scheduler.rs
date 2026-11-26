use esp_hal::rtc_cntl::Rtc;
use esp_println::println;
use khvylyna_core::Settings;

/// Represents calendar date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Computes UTC civil date and time from Unix epoch seconds using Howard Hinnant's algorithm.
pub fn civil_from_timestamp(epoch_secs: u64) -> CivilTime {
    let secs_of_day = (epoch_secs % 86400) as u32;
    let hour = (secs_of_day / 3600) as u8;
    let minute = ((secs_of_day % 3600) / 60) as u8;
    let second = (secs_of_day % 60) as u8;

    let days = (epoch_secs / 86400) as i64 + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = if month <= 2 { y + 1 } else { y } as i32;

    CivilTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

/// Day of week: 0 = Sunday, 1 = Monday, ..., 6 = Saturday.
fn day_of_week(year: i32, month: u8, day: u8) -> u8 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + day as i32) % 7) as u8
}

/// Determines whether daylight saving time (EEST, UTC+3) is active in Ukraine.
/// DST runs from the last Sunday of March (03:00) to the last Sunday of October (04:00).
pub fn is_ukraine_dst(utc: CivilTime) -> bool {
    if utc.month < 3 || utc.month > 10 {
        return false;
    }
    if utc.month > 3 && utc.month < 10 {
        return true;
    }

    // Last Sunday of March / October
    let dow = day_of_week(utc.year, utc.month, utc.day);
    let days_in_month = 31;
    let days_to_end = days_in_month - utc.day;

    if utc.month == 3 {
        // Active from last Sunday at 01:00 UTC (03:00 Kyiv)
        if days_to_end >= 7 {
            false
        } else if dow == 0 {
            utc.hour >= 1
        } else {
            utc.day > (31 - dow)
        }
    } else {
        // Ends on last Sunday at 01:00 UTC (04:00 Kyiv)
        if days_to_end >= 7 {
            true
        } else if dow == 0 {
            utc.hour < 1
        } else {
            utc.day < (31 - dow)
        }
    }
}

/// Converts UTC epoch seconds to Ukraine local time (Kyiv: UTC+2 or UTC+3 with DST).
pub fn ukraine_local_time(epoch_secs: u64) -> CivilTime {
    let utc = civil_from_timestamp(epoch_secs);
    let offset_hours = if is_ukraine_dst(utc) { 3 } else { 2 };
    civil_from_timestamp(epoch_secs + offset_hours * 3600)
}

/// Tracks daily ceremony execution to avoid multiple runs on the same date.
pub struct DailyScheduler {
    pub last_ceremony_day: Option<u8>,
}

impl DailyScheduler {
    pub const fn new() -> Self {
        Self {
            last_ceremony_day: None,
        }
    }

    /// Checks if ceremony should trigger at this moment according to settings.
    pub fn should_trigger(&mut self, epoch_secs: u64, settings: &Settings) -> bool {
        if !settings.ceremony_enabled {
            return false;
        }

        let local = ukraine_local_time(epoch_secs);

        // Don't trigger if already ran today
        if self.last_ceremony_day == Some(local.day) {
            return false;
        }

        // Exactly at 09:00:00
        let is_exact_09_00 = local.hour == 9 && local.minute == 0 && local.second == 0;

        // Grace period check for late device power-on
        let is_within_grace =
            local.hour == 9 && (local.minute as u32) < settings.late_start_grace_minutes;

        if is_exact_09_00 || is_within_grace {
            self.last_ceremony_day = Some(local.day);
            println!(
                "Ceremony triggered for date: {:04}-{:02}-{:02} at {:02}:{:02}:{:02}",
                local.year, local.month, local.day, local.hour, local.minute, local.second
            );
            return true;
        }

        false
    }
}
