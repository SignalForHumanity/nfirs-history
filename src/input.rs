//! Locating and opening input files: transaction files (plain or zipped,
//! singly or in folders) and public data release directories.

use crate::model::{Incident, Key};
use crate::pdr;
use crate::txn::{Filter, TxnReader};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub struct InputError(pub String);

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InputError {}

/// Decode bytes as UTF-8, falling back to Latin-1 (the PDR's encoding).
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Stream a text file line by line without loading it into memory.
pub fn for_each_line(
    path: &Path,
    mut f: impl FnMut(&str) -> Result<(), InputError>,
) -> Result<(), InputError> {
    let file = File::open(path).map_err(|e| InputError(format!("{}: {e}", path.display())))?;
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut buf = Vec::with_capacity(4096);
    loop {
        buf.clear();
        let n = reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| InputError(format!("{}: {e}", path.display())))?;
        if n == 0 {
            return Ok(());
        }
        while matches!(buf.last(), Some(b'\n' | b'\r')) {
            buf.pop();
        }
        if buf.is_empty() {
            continue;
        }
        match std::str::from_utf8(&buf) {
            Ok(s) => f(s)?,
            Err(_) => f(&decode(&buf))?,
        }
    }
}

/// Read `r` and return its text if it is an NFIRS 5.0 transaction file.
/// Only the first 64 KiB are read to decide, so a multi-gigabyte file that is
/// not a transaction file is never loaded into memory.
pub fn read_if_transaction(mut r: impl Read) -> std::io::Result<Option<String>> {
    const SNIFF: u64 = 64 * 1024;
    let mut bytes = Vec::new();
    (&mut r).take(SNIFF).read_to_end(&mut bytes)?;
    // Judge only complete lines, so a multi-byte character cut at the window
    // edge cannot force a Latin-1 decode of an otherwise UTF-8 file.
    let head = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(i) if bytes.len() as u64 == SNIFF => &bytes[..i],
        _ => &bytes[..],
    };
    if !TxnReader::looks_like_transaction_file(&decode(head)) {
        return Ok(None);
    }
    r.read_to_end(&mut bytes)?;
    Ok(Some(decode(&bytes)))
}

/// Result of loading all inputs.
#[derive(Debug, Default)]
pub struct Loaded {
    pub incidents: Vec<Incident>,
    pub warnings: Vec<String>,
    pub files_read: usize,
}

fn is_zip(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && &magic == b"PK\x03\x04"
}

fn files_in(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), InputError> {
    let rd = std::fs::read_dir(dir).map_err(|e| InputError(format!("{}: {e}", dir.display())))?;
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_in(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

struct Loader {
    txn: TxnReader,
    pdr: BTreeMap<Key, Incident>,
    filter: Filter,
    warnings: Vec<String>,
    files_read: usize,
}

impl Loader {
    fn text(&mut self, name: &str, text: Option<String>, explicit: bool) {
        if let Some(text) = text {
            self.txn.read_text(&text);
            self.files_read += 1;
        } else if explicit {
            self.warnings.push(format!(
                "{name}: not an NFIRS 5.0 transaction file; skipped"
            ));
        }
    }

    fn zip(&mut self, path: &Path) -> Result<(), InputError> {
        let err = |e: &dyn fmt::Display| InputError(format!("{}: {e}", path.display()));
        let file = File::open(path).map_err(|e| err(&e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| err(&e))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| err(&e))?;
            if !entry.is_file() {
                continue;
            }
            let name = format!("{}!{}", path.display(), entry.name());
            let mut magic = Vec::new();
            (&mut entry)
                .take(4)
                .read_to_end(&mut magic)
                .map_err(|e| err(&e))?;
            if magic == b"PK\x03\x04" {
                self.warnings
                    .push(format!("{name}: nested ZIP; unzip it first"));
                continue;
            }
            let text = read_if_transaction(magic.as_slice().chain(entry)).map_err(|e| err(&e))?;
            self.text(&name, text, false);
        }
        Ok(())
    }

    fn file(&mut self, path: &Path, explicit: bool) -> Result<(), InputError> {
        if is_zip(path) {
            return self.zip(path);
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !explicit
            && matches!(
                ext.as_str(),
                "pdf" | "doc" | "docx" | "xls" | "xlsx" | "csv"
            )
        {
            return Ok(());
        }
        let text = File::open(path)
            .and_then(read_if_transaction)
            .map_err(|e| InputError(format!("{}: {e}", path.display())))?;
        self.text(&path.display().to_string(), text, explicit);
        Ok(())
    }

    fn path(&mut self, path: &Path) -> Result<(), InputError> {
        if !path.exists() {
            return Err(InputError(format!(
                "{}: no such file or folder",
                path.display()
            )));
        }
        if pdr::is_pdr_dir(path) {
            if self.filter.state.is_none() || self.filter.fdid.is_none() {
                return Err(InputError(format!(
                    "{}: this is NFIRS public data for every department; pass --state and --fdid",
                    path.display()
                )));
            }
            let mut warns = Vec::new();
            pdr::read_dir(path, &self.filter, &mut self.pdr, &mut |w| warns.push(w))?;
            self.warnings.extend(warns);
            self.files_read += 1;
            return Ok(());
        }
        if path.is_dir() {
            let mut files = Vec::new();
            files_in(path, &mut files)?;
            for f in files {
                self.file(&f, false)?;
            }
            return Ok(());
        }
        self.file(path, true)
    }
}

/// Load every input path into one de-duplicated incident list, sorted by
/// date and incident number.
pub fn load(paths: &[PathBuf], filter: &Filter) -> Result<Loaded, InputError> {
    let mut l = Loader {
        txn: TxnReader::new(filter.clone()),
        pdr: BTreeMap::new(),
        filter: filter.clone(),
        warnings: Vec::new(),
        files_read: 0,
    };
    for p in paths {
        l.path(p)?;
    }
    let s = &l.txn.stats;
    if s.malformed > 0 {
        l.warnings.push(format!(
            "{} malformed transaction records skipped",
            s.malformed
        ));
    }
    let mut all = l.pdr;
    // Transaction files are the department's own records; prefer them over
    // the public release when both describe the same incident. A transaction
    // incident without a Basic Incident record (a change-only file) carries
    // no incident type or times, so it must not replace a public record.
    for (key, inc) in std::mem::take(&mut l.txn.incidents) {
        if inc.incident_type.is_empty() && all.contains_key(&key) {
            l.warnings.push(format!(
                "incident {} {}: transaction file has no Basic Incident record; kept the public data version",
                key.date, inc.number
            ));
            continue;
        }
        all.insert(key, inc);
    }
    let mut incidents: Vec<Incident> = all.into_values().collect();
    incidents.sort_by(|a, b| {
        (a.date, &a.number, a.exposure, &a.fdid).cmp(&(b.date, &b.number, b.exposure, &b.fdid))
    });
    Ok(Loaded {
        incidents,
        warnings: l.warnings,
        files_read: l.files_read,
    })
}
