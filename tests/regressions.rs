//! Regression tests for review findings. No network needed.

use nfirs_history::input::load;
use nfirs_history::txn::{Filter, TxnReader};
use std::path::PathBuf;

/// A Basic Incident (1005) record for `key` (elements 1-5 plus trailing `^`).
fn basic(key: &str, tt: &str, inc_type: &str) -> String {
    let mut v = vec![String::new(); 31];
    v[0] = inc_type.into();
    v[2] = "N".into();
    v[3] = "202301011200".into();
    v[4] = "202301011207".into();
    format!("{key}1005^{tt}^{}^", v.join("^"))
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nfirs-history-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Spec 2015.1 p.135: "If the Basic Incident Transaction is deleted, the
/// entire incident is deleted. (Including any exposure record for fire
/// incidents)".
#[test]
fn deleting_basic_incident_deletes_its_exposures() {
    let k0 = "11001^NY^20230101^0000001^000^";
    let k1 = "11001^NY^20230101^0000001^001^";
    let text = format!("^\n{}\n{}\n", basic(k0, "", "111"), basic(k1, "", "111"));
    let mut r = TxnReader::new(Filter::default());
    r.read_text(&text);
    assert_eq!(r.incidents.len(), 2);
    r.read_text(&format!("^\n{k0}1005^2^\n"));
    assert!(
        r.incidents.is_empty(),
        "exposure 1 survived: {:?}",
        r.incidents.keys()
    );
}

/// Deleting a child record of an incident that was never loaded must not
/// invent an empty incident.
#[test]
fn deleting_child_record_of_unknown_incident_creates_nothing() {
    let mut r = TxnReader::new(Filter::default());
    r.read_text("^\n11001^NY^20230101^0000009^000^1010^2^\n");
    assert!(r.incidents.is_empty());
}

/// A change-only transaction file (just an address change) must not wipe the
/// incident type and times that came from the public data release.
#[test]
fn change_only_transaction_does_not_erase_pdr_incident() {
    let dir = tmpdir("merge");
    let pdr = dir.join("pdr2023");
    std::fs::create_dir_all(&pdr).unwrap();
    std::fs::write(
        pdr.join("basicincident.txt"),
        "STATE^FDID^INC_DATE^INC_NO^EXP_NO^INC_TYPE^AID^ALARM^ARRIVAL\n\
         NY^11001^01012023^0000001^0^111^N^010120231200^010120231207\n",
    )
    .unwrap();
    let txn = dir.join("change.txt");
    std::fs::write(
        &txn,
        "^\nV^S^\n11001^NY^20230101^0000001^000^1010^1^^1^12^^MAIN^ST^^^TOWN^NY^12345^^\n",
    )
    .unwrap();
    let filter = Filter {
        state: Some("NY".into()),
        fdid: Some("11001".into()),
    };
    let loaded = load(&[pdr, txn], &filter).unwrap();
    assert_eq!(loaded.incidents.len(), 1);
    let inc = &loaded.incidents[0];
    assert_eq!(inc.incident_type, "111");
    assert_eq!(inc.response_minutes(), Some(7));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A large non-transaction file in a scanned folder (for example a public
/// data table several GB in size) must be sniffed, not read into memory.
#[test]
fn large_non_transaction_files_are_not_read_whole() {
    use nfirs_history::input::read_if_transaction;
    struct Endless(usize);
    impl std::io::Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            assert!(self.0 < 16 << 20, "read past the sniff window");
            let line = b"NY^11001^01012019^0000001^0^5.0^001^111^N\n";
            let n = buf.len().min(line.len());
            buf[..n].copy_from_slice(&line[..n]);
            self.0 += n;
            Ok(n)
        }
    }
    let header = std::io::Read::chain(
        &b"STATE^FDID^INC_DATE^INC_NO^EXP_NO^VERSION^DEPT_STA^INC_TYPE^AID\n"[..],
        Endless(0),
    );
    assert!(read_if_transaction(header).unwrap().is_none());

    let txn = "^\nV^S^\n11001^NY^20230101^1^0^1000^^^5.0^\n";
    assert_eq!(
        read_if_transaction(txn.as_bytes()).unwrap().as_deref(),
        Some(txn)
    );
}
