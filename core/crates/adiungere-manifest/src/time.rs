//! Instants, written down with where they came from.
//!
//! A recording carries several clocks and they do not agree: the container header, the file name, the
//! satellite sentences in the vendor box. None of them is trusted on its own, so a time in a manifest is
//! never a bare value: it is a value, the clock it was read from, and a note on what that clock is known
//! to mean. Every sentence the product forms about a time says "no later than" and names the clock.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The closed set of clocks a time can be read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeSource {
    /// The creation time in the container's movie header, written by the recorder.
    ContainerClock,
    /// A satellite sentence in the vendor telemetry.
    SatelliteClock,
    /// The recording's file name, under a naming grammar.
    FileName,
    /// The system clock of the machine producing the manifest.
    SystemClockAtManifest,
}

impl TimeSource {
    /// The words a sentence uses to name this clock.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::ContainerClock => "the container clock",
            Self::SatelliteClock => "the satellite clock",
            Self::FileName => "the file name",
            Self::SystemClockAtManifest => "the system clock at the time of the manifest",
        }
    }
}

/// A time, with its clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeValue {
    /// The instant, as `YYYY-MM-DDTHH:MM:SSZ` when the clock is known to be UTC, and as
    /// `YYYY-MM-DDTHH:MM:SS` with no designator when it is not.
    pub value: String,
    /// The clock the instant was read from.
    pub source: TimeSource,
    /// What is known about that clock, in one sentence.
    pub note: String,
}

/// A civil date and time, without a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Civil {
    /// The year.
    pub year: i64,
    /// The month, one to twelve.
    pub month: u8,
    /// The day, one to thirty-one.
    pub day: u8,
    /// The hour, zero to twenty-three.
    pub hour: u8,
    /// The minute.
    pub minute: u8,
    /// The second.
    pub second: u8,
}

impl Civil {
    /// The civil time that many seconds after 1970-01-01T00:00:00.
    #[must_use]
    pub fn from_unix_seconds(seconds: i64) -> Self {
        let days = seconds.div_euclid(86_400);
        let remainder = seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let hour = u8::try_from(remainder.div_euclid(3600)).unwrap_or(0);
        let minute = u8::try_from(remainder.rem_euclid(3600).div_euclid(60)).unwrap_or(0);
        let second = u8::try_from(remainder.rem_euclid(60)).unwrap_or(0);
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// The civil time that many seconds after 1904-01-01T00:00:00, which is the container's epoch.
    #[must_use]
    pub fn from_container_seconds(seconds: u64) -> Self {
        const OFFSET: i64 = 2_082_844_800;
        let unix = i64::try_from(seconds).unwrap_or(i64::MAX).saturating_sub(OFFSET);
        Self::from_unix_seconds(unix)
    }

    /// The instant as `YYYY-MM-DDTHH:MM:SS`, with no zone designator.
    #[must_use]
    pub fn unzoned(&self) -> String {
        self.to_string()
    }

    /// The instant as `YYYY-MM-DDTHH:MM:SSZ`.
    #[must_use]
    pub fn utc(&self) -> String {
        format!("{self}Z")
    }
}

impl fmt::Display for Civil {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Days since 1970-01-01 to a civil date, by the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    // Every quotient below is of a non-negative value, so Euclidean division is plain division.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era.div_euclid(1460) + day_of_era.div_euclid(36_524)
        - day_of_era.div_euclid(146_096))
    .div_euclid(365);
    let year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era.div_euclid(4) - year_of_era.div_euclid(100));
    let shifted_month = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * shifted_month + 2).div_euclid(5) + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (
        year,
        u8::try_from(month).unwrap_or(1),
        u8::try_from(day).unwrap_or(1),
    )
}

/// The system clock now, as UTC.
#[must_use]
pub fn now_utc() -> Civil {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX));
    Civil::from_unix_seconds(seconds)
}

#[cfg(test)]
mod tests {
    use super::Civil;

    #[test]
    fn the_unix_epoch_is_the_first_of_january_1970() {
        // Act
        let civil = Civil::from_unix_seconds(0);

        // Assert
        assert_eq!(civil.utc(), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn a_known_instant_converts_correctly() {
        // 2026-01-01T00:00:00Z is 1767225600 seconds after the epoch.

        // Act
        let civil = Civil::from_unix_seconds(1_767_225_600);

        // Assert
        assert_eq!(civil.unzoned(), "2026-01-01T00:00:00");
    }

    #[test]
    fn a_leap_day_and_an_end_of_year_convert_correctly() {
        // 2024-02-29T12:34:56Z is 1709210096; 2023-12-31T23:59:59Z is 1704067199.

        // Act
        let leap = Civil::from_unix_seconds(1_709_210_096);
        let year_end = Civil::from_unix_seconds(1_704_067_199);

        // Assert
        assert_eq!(leap.utc(), "2024-02-29T12:34:56Z");
        assert_eq!(year_end.utc(), "2023-12-31T23:59:59Z");
    }

    #[test]
    fn the_container_epoch_is_1904() {
        // Act
        let civil = Civil::from_container_seconds(0);

        // Assert
        assert_eq!(civil.unzoned(), "1904-01-01T00:00:00");
    }

    #[test]
    fn an_instant_before_the_unix_epoch_converts_correctly() {
        // Act
        let civil = Civil::from_unix_seconds(-1);

        // Assert
        assert_eq!(civil.utc(), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn the_edges_of_the_four_hundred_year_cycle_and_of_its_centuries_convert_correctly() {
        // The reference values were computed with a separate calendar implementation. A century year is a
        // leap year only every four hundred years, and the cycle is counted from the first of March.

        // Act and assert
        for (seconds, expected) in [
            (-11_670_912_000, "1600-03-01T00:00:00Z"),
            (-2_203_891_201, "1900-02-28T23:59:59Z"),
            (-2_203_891_200, "1900-03-01T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),
            (951_868_800, "2000-03-01T00:00:00Z"),
            (4_107_456_000, "2100-02-28T00:00:00Z"),
            (4_107_542_400, "2100-03-01T00:00:00Z"),
            (7_258_032_000, "2199-12-31T00:00:00Z"),
            (10_413_792_000, "2300-01-01T00:00:00Z"),
            (13_569_379_200, "2399-12-31T00:00:00Z"),
            (13_574_649_599, "2400-02-29T23:59:59Z"),
            (13_574_649_600, "2400-03-01T00:00:00Z"),
        ] {
            assert_eq!(Civil::from_unix_seconds(seconds).utc(), expected, "{seconds}");
        }
    }

    #[test]
    fn every_clock_is_described_in_different_words() {
        // Act
        let words: Vec<&str> = [
            super::TimeSource::ContainerClock,
            super::TimeSource::SatelliteClock,
            super::TimeSource::FileName,
            super::TimeSource::SystemClockAtManifest,
        ]
        .iter()
        .map(|source| source.describe())
        .collect();

        // Assert
        assert!(words.iter().all(|text| text.starts_with("the ")), "{words:?}");
        let distinct: std::collections::BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(distinct.len(), 4, "{words:?}");
    }
}
