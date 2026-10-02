//! Reader for NFIRS 5.0 incident transaction files ("flat file transfer
//! format", NFIRS 5.0 Design Documentation, Specification Release 2015.1).
//!
//! The file's first record is the field delimiter (usually `^`), the second
//! is the vendor/software ID, and every following record starts with the
//! five key fields, a record type and a transaction type. Only the record
//! types needed for incident listings and AFG statistics are decoded; all
//! others are counted and skipped.

use crate::model::{
    Date, DateTime, Incident, Key, fire_module_acres, join_address, parse_float, parse_int,
    same_fdid,
};
use std::collections::BTreeMap;

/// Which department(s) to keep. Empty fields match everything.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub state: Option<String>,
    pub fdid: Option<String>,
}

impl Filter {
    pub fn matches(&self, state: &str, fdid: &str) -> bool {
        self.state
            .as_deref()
            .is_none_or(|s| s.trim().eq_ignore_ascii_case(state.trim()))
            && self.fdid.as_deref().is_none_or(|f| same_fdid(f, fdid))
    }
}

#[derive(Debug, Default)]
pub struct Stats {
    pub records: usize,
    pub skipped_other_types: usize,
    pub malformed: usize,
    pub filtered_out: usize,
}

/// Accumulates incidents across one or more transaction files.
#[derive(Debug, Default)]
pub struct TxnReader {
    pub incidents: BTreeMap<Key, Incident>,
    pub stats: Stats,
    filter: Filter,
}

/// Guess the delimiter from the first line of a file.
fn detect_delimiter(first: &str) -> Option<String> {
    let line = first.trim_end_matches(['\r', '\n']);
    if line.is_empty() || line.len() > 6 || line.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    // The record may itself be terminated by the delimiter ("^^").
    let half = line.len() / 2;
    if line.len().is_multiple_of(2) && line.is_char_boundary(half) && line[..half] == line[half..] {
        return Some(line[..half].to_string());
    }
    Some(line.to_string())
}

fn f(fields: &[&str], element: usize) -> String {
    fields
        .get(element - 1)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn multi(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

impl TxnReader {
    pub fn new(filter: Filter) -> Self {
        TxnReader {
            filter,
            ..Default::default()
        }
    }

    /// Returns true if the text looks like an NFIRS 5.0 transaction file.
    pub fn looks_like_transaction_file(text: &str) -> bool {
        let mut lines = text.lines();
        let delim = lines
            .next()
            .and_then(detect_delimiter)
            .unwrap_or_else(|| "^".into());
        text.lines().take(20).any(|l| {
            let fields: Vec<&str> = l.split(delim.as_str()).collect();
            fields.len() >= 7 && fields[5].trim().len() == 4 && fields[5].trim().starts_with('1')
        })
    }

    /// Read one transaction file's text.
    pub fn read_text(&mut self, text: &str) {
        let mut lines = text.lines().peekable();
        let delim = match lines.peek().and_then(|l| detect_delimiter(l)) {
            Some(d) => {
                lines.next();
                d
            }
            None => "^".to_string(),
        };
        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split(delim.as_str()).collect();
            self.record(&fields);
        }
    }

    fn record(&mut self, fields: &[&str]) {
        if fields.len() < 7 {
            // The vendor/software ID record has two fields; anything else this
            // short is not an incident record.
            if fields.len() > 3 {
                self.stats.malformed += 1;
            }
            return;
        }
        let record_type: u32 = match fields[5].trim().parse() {
            Ok(n) => n,
            Err(_) => {
                self.stats.malformed += 1;
                return;
            }
        };
        if !(1000..2000).contains(&record_type) {
            // 2000-series department records, 7000/8000 local and state extensions.
            self.stats.skipped_other_types += 1;
            return;
        }
        let state = f(fields, 2).to_ascii_uppercase();
        let fdid = f(fields, 1);
        if !self.filter.matches(&state, &fdid) {
            self.stats.filtered_out += 1;
            return;
        }
        let key = Key::new(&state, &fdid, &f(fields, 3), &f(fields, 4), &f(fields, 5));
        let transaction = f(fields, 7);
        self.stats.records += 1;

        if transaction == "2" && matches!(record_type, 1000 | 1005) {
            // Deleting the Basic Incident deletes the whole incident,
            // including its exposures when exposure 0 is deleted (spec p.135).
            if key.exposure == 0 {
                self.incidents.retain(|k, _| {
                    (&k.state, &k.fdid, &k.date, &k.number)
                        != (&key.state, &key.fdid, &key.date, &key.number)
                });
            } else {
                self.incidents.remove(&key);
            }
            return;
        }
        if transaction == "3" {
            // "No activity" header: a placeholder, not an incident.
            return;
        }
        let deleting = transaction == "2";
        if !matches!(record_type, 1000 | 1005 | 1010 | 1100 | 1300) {
            self.stats.skipped_other_types += 1;
            return;
        }
        if deleting && !self.incidents.contains_key(&key) {
            // Deleting part of an incident we never saw: nothing to do.
            return;
        }
        let inc = self
            .incidents
            .entry(key.clone())
            .or_insert_with(|| Incident {
                state,
                fdid,
                date: Date::parse(&f(fields, 3)),
                number: f(fields, 4),
                exposure: key.exposure,
                ..Default::default()
            });
        match record_type {
            1000 => inc.station = f(fields, 8),
            1005 => {
                inc.incident_type = f(fields, 8);
                inc.aid = f(fields, 10);
                inc.alarm = DateTime::parse(&f(fields, 11));
                inc.arrival = DateTime::parse(&f(fields, 12));
                inc.controlled = DateTime::parse(&f(fields, 13));
                inc.cleared = DateTime::parse(&f(fields, 14));
                inc.actions = multi(&f(fields, 18));
                inc.property_loss = parse_int(&f(fields, 27));
                inc.contents_loss = parse_int(&f(fields, 28));
                inc.ff_deaths = parse_int(&f(fields, 31));
                inc.other_deaths = parse_int(&f(fields, 32));
                inc.ff_injuries = parse_int(&f(fields, 33));
                inc.other_injuries = parse_int(&f(fields, 34));
                inc.property_use = f(fields, 38);
                if inc.date.is_none() {
                    inc.date = inc.alarm.map(|a| a.date);
                }
            }
            1010 => {
                if deleting {
                    inc.address.clear();
                    inc.city.clear();
                    inc.zip.clear();
                } else {
                    inc.address = join_address(
                        &[
                            &f(fields, 10),
                            &f(fields, 11),
                            &f(fields, 12),
                            &f(fields, 13),
                            &f(fields, 14),
                        ],
                        &f(fields, 15),
                        &f(fields, 19),
                    );
                    inc.city = f(fields, 16);
                    inc.zip = f(fields, 18);
                }
            }
            1100 => {
                inc.fire_acres = if deleting {
                    None
                } else {
                    fire_module_acres(&f(fields, 11), &f(fields, 12))
                };
            }
            1300 => {
                inc.wildland_acres = if deleting {
                    None
                } else {
                    parse_float(&f(fields, 35))
                };
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic(key: &str, tt: &str, inc_type: &str, aid: &str) -> String {
        // Elements 1-7 are in `key`; this fills 8-38 of the 1005 record.
        let mut v = vec![String::new(); 31];
        v[0] = inc_type.into();
        v[2] = aid.into();
        v[3] = "202301011200".into();
        v[4] = "202301011207".into();
        format!("{key}1005^{tt}^{}^", v.join("^"))
    }

    #[test]
    fn delimiter_detection() {
        assert_eq!(detect_delimiter("^"), Some("^".into()));
        assert_eq!(detect_delimiter("^^\r"), Some("^".into()));
        assert_eq!(detect_delimiter("|~|"), Some("|~|".into()));
        assert_eq!(detect_delimiter("12S22R69K^1234C^"), None);
    }

    #[test]
    fn add_change_delete() {
        let k = "11001^NY^20230101^0000001^000^";
        let text = format!(
            "^\r\nVEND^SW^\r\n{k}1000^^001^5.0^\r\n{}\r\n{}\r\n",
            basic(k, "", "111", "N"),
            basic(k, "1", "113", "N"),
        );
        let mut r = TxnReader::new(Filter::default());
        r.read_text(&text);
        assert_eq!(r.incidents.len(), 1);
        let inc = r.incidents.values().next().unwrap();
        assert_eq!(inc.incident_type, "113");
        assert_eq!(inc.station, "001");
        assert_eq!(inc.response_minutes(), Some(7));

        r.read_text(&format!("^\n{k}1005^2^\n"));
        assert!(r.incidents.is_empty());
    }

    #[test]
    fn filter_and_no_activity() {
        let text = format!(
            "^\n11001^NY^20230101^0000000^000^1000^3^^5.0^\n{}\n{}\n",
            basic("11001^NY^20230102^0000002^000^", "", "321", "N"),
            basic("22002^NY^20230102^0000002^000^", "", "321", "N"),
        );
        let mut r = TxnReader::new(Filter {
            state: Some("ny".into()),
            fdid: Some("11001".into()),
        });
        r.read_text(&text);
        assert_eq!(r.incidents.len(), 1);
        assert_eq!(r.stats.filtered_out, 1);
    }

    #[test]
    fn detection() {
        assert!(TxnReader::looks_like_transaction_file(
            "^\nV^S^\n11001^NY^20230101^1^0^1000^^^5.0^\n"
        ));
        assert!(!TxnReader::looks_like_transaction_file(
            "hello,world\n1,2\n"
        ));
    }
}
