//! Reader for the NFIRS Public Data Release (PDR) published on OpenFEMA.
//!
//! Each year is a set of caret-delimited text tables with a header row
//! (2012 and later; earlier years are dBASE and not supported). Files are
//! streamed line by line, so multi-gigabyte tables are filtered to one
//! department in constant memory.

use crate::input::{InputError, for_each_line};
use crate::model::{
    Date, DateTime, Incident, Key, fire_module_acres, join_address, parse_float, parse_int,
};
use crate::txn::Filter;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub const BASIC: &str = "basicincident.txt";
const ADDRESS: &str = "incidentaddress.txt";
const FIRE: &str = "fireincident.txt";
const WILDLAND: &str = "wildlands.txt";

/// Find a PDR table by file name (case-insensitive) in `dir` or one level below.
pub fn find_table(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut subdirs = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
        {
            return Some(p);
        }
    }
    subdirs.sort();
    subdirs.into_iter().find_map(|d| {
        std::fs::read_dir(&d)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
    })
}

pub fn is_pdr_dir(dir: &Path) -> bool {
    dir.is_dir() && find_table(dir, BASIC).is_some()
}

struct Header(HashMap<String, usize>);

impl Header {
    fn parse(line: &str) -> Header {
        Header(
            line.split('^')
                .enumerate()
                .map(|(i, n)| (clean(n).to_ascii_uppercase(), i))
                .collect(),
        )
    }
    fn get<'a>(&self, fields: &[&'a str], name: &str) -> &'a str {
        self.0
            .get(name)
            .and_then(|&i| fields.get(i))
            .map(|s| clean(s))
            .unwrap_or("")
    }
    fn require(&self, names: &[&str], file: &Path) -> Result<(), InputError> {
        for n in names {
            if !self.0.contains_key(*n) {
                return Err(InputError(format!(
                    "{}: column {n} not found in header; is this an NFIRS public data file?",
                    file.display()
                )));
            }
        }
        Ok(())
    }
}

fn clean(s: &str) -> &str {
    s.trim().trim_matches('"').trim()
}

/// Stream one PDR table, calling `row` for each record of the selected department.
fn scan(
    path: &Path,
    filter: &Filter,
    mut row: impl FnMut(&Header, &[&str], Key),
) -> Result<usize, InputError> {
    let mut header: Option<Header> = None;
    let mut matched = 0;
    for_each_line(path, |line| {
        let fields: Vec<&str> = line.split('^').collect();
        let Some(h) = header.as_ref() else {
            let h = Header::parse(line);
            h.require(&["STATE", "FDID", "INC_DATE", "INC_NO", "EXP_NO"], path)?;
            header = Some(h);
            return Ok(());
        };
        let state = h.get(&fields, "STATE");
        let fdid = h.get(&fields, "FDID");
        if !filter.matches(state, fdid) {
            return Ok(());
        }
        matched += 1;
        let key = Key::new(
            state,
            fdid,
            h.get(&fields, "INC_DATE"),
            h.get(&fields, "INC_NO"),
            h.get(&fields, "EXP_NO"),
        );
        row(h, &fields, key);
        Ok(())
    })?;
    Ok(matched)
}

/// Read one PDR year directory for the department in `filter`.
pub fn read_dir(
    dir: &Path,
    filter: &Filter,
    out: &mut BTreeMap<Key, Incident>,
    warn: &mut dyn FnMut(String),
) -> Result<(), InputError> {
    let basic = find_table(dir, BASIC)
        .ok_or_else(|| InputError(format!("{}: no {BASIC} found", dir.display())))?;
    let mut found: BTreeMap<Key, Incident> = BTreeMap::new();
    scan(&basic, filter, |h, f, key| {
        let alarm = DateTime::parse(h.get(f, "ALARM"));
        let actions = ["ACT_TAK1", "ACT_TAK2", "ACT_TAK3"]
            .iter()
            .map(|c| h.get(f, c).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let inc = Incident {
            state: key.state.clone(),
            fdid: h.get(f, "FDID").to_string(),
            date: Date::parse(h.get(f, "INC_DATE")).or(alarm.map(|a| a.date)),
            number: h.get(f, "INC_NO").to_string(),
            exposure: key.exposure,
            station: h.get(f, "DEPT_STA").to_string(),
            incident_type: h.get(f, "INC_TYPE").to_string(),
            aid: h.get(f, "AID").to_string(),
            alarm,
            arrival: DateTime::parse(h.get(f, "ARRIVAL")),
            controlled: DateTime::parse(h.get(f, "INC_CONT")),
            cleared: DateTime::parse(h.get(f, "LU_CLEAR")),
            actions,
            property_use: h.get(f, "PROP_USE").to_string(),
            property_loss: parse_int(h.get(f, "PROP_LOSS")),
            contents_loss: parse_int(h.get(f, "CONT_LOSS")),
            ff_deaths: parse_int(h.get(f, "FF_DEATH")),
            other_deaths: parse_int(h.get(f, "OTH_DEATH")),
            ff_injuries: parse_int(h.get(f, "FF_INJ")),
            other_injuries: parse_int(h.get(f, "OTH_INJ")),
            ..Default::default()
        };
        found.insert(key, inc);
    })?;
    if found.is_empty() {
        warn(format!(
            "{}: no incidents for this state/FDID in {}",
            dir.display(),
            basic.display()
        ));
        return Ok(());
    }

    match find_table(dir, ADDRESS) {
        Some(p) => {
            scan(&p, filter, |h, f, key| {
                if let Some(inc) = found.get_mut(&key) {
                    inc.address = join_address(
                        &[
                            h.get(f, "NUM_MILE"),
                            h.get(f, "STREET_PRE"),
                            h.get(f, "STREETNAME"),
                            h.get(f, "STREETTYPE"),
                            h.get(f, "STREETSUF"),
                        ],
                        h.get(f, "APT_NO"),
                        h.get(f, "X_STREET"),
                    );
                    inc.city = h.get(f, "CITY").to_string();
                    inc.zip = h.get(f, "ZIP5").to_string();
                }
            })?;
        }
        None => warn(format!(
            "{}: no {ADDRESS}; addresses will be blank",
            dir.display()
        )),
    }
    for (table, wildland) in [(FIRE, false), (WILDLAND, true)] {
        match find_table(dir, table) {
            Some(p) => {
                scan(&p, filter, |h, f, key| {
                    if let Some(inc) = found.get_mut(&key) {
                        if wildland {
                            inc.wildland_acres = parse_float(h.get(f, "ACRES_BURN"));
                        } else {
                            inc.fire_acres =
                                fire_module_acres(h.get(f, "ACRES_BURN"), h.get(f, "LESS_1ACRE"));
                        }
                    }
                })?;
            }
            None => warn(format!(
                "{}: no {table}; vegetation-fire acreage may be incomplete",
                dir.display()
            )),
        }
    }
    out.extend(found);
    Ok(())
}
