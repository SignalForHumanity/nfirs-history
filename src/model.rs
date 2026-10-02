//! Incident model shared by both input formats.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub hour: u32,
    pub minute: u32,
}

fn valid_ymd(year: i32, month: u32, day: u32) -> bool {
    (1970..=2100).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn num(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

impl Date {
    /// Parse an 8-digit date. The NFIRS 5.0 transaction spec uses `YYYYMMDD`;
    /// the public data release uses `MMDDYYYY`. The two never collide because
    /// a month cannot be 19 or 20 and a year cannot start with 0 or 1.
    pub fn parse(s: &str) -> Option<Date> {
        let s = s.trim();
        if s.len() != 8 {
            return None;
        }
        let ymd = (num(&s[0..4])? as i32, num(&s[4..6])?, num(&s[6..8])?);
        if valid_ymd(ymd.0, ymd.1, ymd.2) {
            return Some(Date {
                year: ymd.0,
                month: ymd.1,
                day: ymd.2,
            });
        }
        let mdy = (num(&s[4..8])? as i32, num(&s[0..2])?, num(&s[2..4])?);
        if valid_ymd(mdy.0, mdy.1, mdy.2) {
            return Some(Date {
                year: mdy.0,
                month: mdy.1,
                day: mdy.2,
            });
        }
        None
    }

    /// Days since 1970-01-01 (proleptic Gregorian).
    pub fn days(&self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

impl DateTime {
    /// Parse `YYYYMMDDHHMM[SS]` or `MMDDYYYYHHMM[SS]`.
    pub fn parse(s: &str) -> Option<DateTime> {
        let s = s.trim();
        if s.len() != 12 && s.len() != 14 {
            return None;
        }
        let date = Date::parse(&s[0..8])?;
        let hour = num(&s[8..10])?;
        let minute = num(&s[10..12])?;
        if hour > 23 || minute > 59 {
            return None;
        }
        Some(DateTime { date, hour, minute })
    }

    pub fn minutes(&self) -> i64 {
        self.date.days() * 1440 + i64::from(self.hour) * 60 + i64::from(self.minute)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:02}:{:02}", self.date, self.hour, self.minute)
    }
}

/// The five NFIRS key fields that identify one incident record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub state: String,
    pub fdid: String,
    pub date: String,
    pub number: String,
    pub exposure: u32,
}

impl Key {
    /// Build a key with normalized parts, so the same incident read from a
    /// transaction file and from the public data release compares equal.
    pub fn new(state: &str, fdid: &str, date: &str, number: &str, exposure: &str) -> Key {
        let strip = |s: &str| {
            let t = s.trim().trim_start_matches('0').to_ascii_uppercase();
            if t.is_empty() && !s.trim().is_empty() {
                "0".to_string()
            } else {
                t
            }
        };
        Key {
            state: state.trim().to_ascii_uppercase(),
            fdid: strip(fdid),
            date: Date::parse(date)
                .map(|d| d.to_string())
                .unwrap_or_else(|| date.trim().to_string()),
            number: strip(number),
            exposure: exposure.trim().parse().unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Incident {
    pub state: String,
    pub fdid: String,
    pub date: Option<Date>,
    pub number: String,
    pub exposure: u32,
    pub station: String,
    pub incident_type: String,
    pub aid: String,
    pub alarm: Option<DateTime>,
    pub arrival: Option<DateTime>,
    pub controlled: Option<DateTime>,
    pub cleared: Option<DateTime>,
    pub actions: Vec<String>,
    pub property_use: String,
    pub property_loss: Option<i64>,
    pub contents_loss: Option<i64>,
    pub ff_deaths: Option<i64>,
    pub other_deaths: Option<i64>,
    pub ff_injuries: Option<i64>,
    pub other_injuries: Option<i64>,
    pub address: String,
    pub city: String,
    pub zip: String,
    /// Acres from the Fire module (whole acres; 0 when "less than one acre").
    pub fire_acres: Option<f64>,
    /// Total acres burned from the Wildland module.
    pub wildland_acres: Option<f64>,
}

impl Incident {
    /// Three-digit national incident type code. Plus-one codes (a fourth,
    /// locally defined digit) roll up to their national parent.
    pub fn type_code(&self) -> Option<u16> {
        let t = self.incident_type.trim();
        if t.len() < 3 {
            return None;
        }
        num(&t[0..3]).map(|n| n as u16)
    }

    /// NFIRS series (1-9), from the first digit of the incident type.
    pub fn series(&self) -> Option<u8> {
        self.type_code()
            .map(|c| (c / 100) as u8)
            .filter(|s| (1..=9).contains(s))
    }

    pub fn year(&self) -> Option<i32> {
        self.date
            .map(|d| d.year)
            .or(self.alarm.map(|a| a.date.year))
    }

    pub fn month(&self) -> Option<u32> {
        self.date
            .map(|d| d.month)
            .or(self.alarm.map(|a| a.date.month))
    }

    /// Aid codes 3, 4 and 5: this department helped another one.
    pub fn is_aid_given(&self) -> bool {
        matches!(self.aid.trim(), "3" | "4" | "5")
    }

    /// Alarm to arrival in whole minutes, if both are present and plausible
    /// (0 to 24 hours).
    pub fn response_minutes(&self) -> Option<i64> {
        let d = self.arrival?.minutes() - self.alarm?.minutes();
        (0..=1440).contains(&d).then_some(d)
    }

    /// Acres burned: the Wildland module figure when present, else the Fire module.
    pub fn acres(&self) -> Option<f64> {
        self.wildland_acres.or(self.fire_acres)
    }
}

/// Parse an integer field, tolerating blanks and stray spaces.
pub fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() { None } else { s.parse().ok() }
}

pub fn parse_float(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
    }
}

/// Fire-module acres: the whole-acre field, or 0 when only the
/// "less than one acre" flag is set.
pub fn fire_module_acres(acres: &str, less_than_one: &str) -> Option<f64> {
    parse_float(acres).or_else(|| {
        let flag = less_than_one.trim();
        (flag.eq_ignore_ascii_case("Y") || flag == "1").then_some(0.0)
    })
}

/// Build a one-line street address from NFIRS address parts.
pub fn join_address(parts: &[&str], apartment: &str, cross: &str) -> String {
    let mut s = parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let apt = apartment.trim();
    if !apt.is_empty() {
        s.push_str(" #");
        s.push_str(apt);
    }
    let cross = cross.trim();
    if !cross.is_empty() {
        if !s.is_empty() {
            s.push_str(" / ");
        }
        s.push_str(cross);
    }
    s
}

/// Compare fire department IDs, ignoring case, whitespace and leading zeros.
pub fn same_fdid(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches('0').to_ascii_uppercase();
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_both_orders() {
        let d = Date::parse("20230704").unwrap();
        assert_eq!(d.to_string(), "2023-07-04");
        assert_eq!(Date::parse("07042023").unwrap(), d);
        assert_eq!(Date::parse("12312019").unwrap().to_string(), "2019-12-31");
        assert!(Date::parse("2023070").is_none());
        assert!(Date::parse("13452023").is_none());
        assert!(Date::parse("").is_none());
    }

    #[test]
    fn datetimes_and_minutes() {
        let a = DateTime::parse("202312312355").unwrap();
        let b = DateTime::parse("01012024000730").unwrap();
        assert_eq!(a.to_string(), "2023-12-31 23:55");
        assert_eq!(b.minutes() - a.minutes(), 12);
        assert!(DateTime::parse("202312312460").is_none());
    }

    #[test]
    fn epoch_days() {
        let d = |y, m, dd| Date {
            year: y,
            month: m,
            day: dd,
        };
        assert_eq!(d(1970, 1, 1).days(), 0);
        assert_eq!(d(2000, 3, 1).days(), 11017);
        assert_eq!(d(2024, 3, 1).days() - d(2024, 2, 28).days(), 2);
    }

    #[test]
    fn type_codes_and_series() {
        let mut i = Incident {
            incident_type: "1111".into(),
            ..Default::default()
        };
        assert_eq!(i.type_code(), Some(111));
        assert_eq!(i.series(), Some(1));
        i.incident_type = "UUU".into();
        assert_eq!(i.series(), None);
        i.incident_type = "052".into();
        assert_eq!(i.series(), None);
    }

    #[test]
    fn fdid_comparison() {
        assert!(same_fdid("01234", "1234"));
        assert!(same_fdid(" 0a123 ", "A123"));
        assert!(!same_fdid("11001", "11002"));
    }

    #[test]
    fn acres_rules() {
        assert_eq!(fire_module_acres("", "Y"), Some(0.0));
        assert_eq!(fire_module_acres("12", "N"), Some(12.0));
        assert_eq!(fire_module_acres("", ""), None);
        assert_eq!(parse_float("-3"), None);
    }

    #[test]
    fn address_join() {
        assert_eq!(
            join_address(&["12", "N", "MAIN", "ST", ""], "2B", ""),
            "12 N MAIN ST #2B"
        );
        assert_eq!(
            join_address(&["", "", "", "", ""], "", "RT 9 AT MILE 4"),
            "RT 9 AT MILE 4"
        );
    }
}
