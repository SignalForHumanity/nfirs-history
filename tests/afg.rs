//! AFG Table 7 numbers from hand-built inputs with hand-checked answers.
//! The transaction file and the public-data folder describe the same
//! incidents, so both must give the same table.

use nfirs_history::input::load;
use nfirs_history::report::{AfgYear, afg_year};
use nfirs_history::txn::Filter;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One transaction record for NY 11001; `elems` sets elements 8 and up.
fn rec(date: &str, num: &str, exp: &str, rtype: &str, tt: &str, elems: &[(usize, &str)]) -> String {
    let len = elems.iter().map(|e| e.0).max().unwrap_or(7).max(7);
    let mut v = vec![String::new(); len];
    for (i, s) in ["11001", "NY", date, num, exp, rtype, tt]
        .iter()
        .enumerate()
    {
        v[i] = s.to_string();
    }
    for (i, s) in elems {
        v[i - 1] = s.to_string();
    }
    v.join("^") + "^"
}

fn basic(date: &str, num: &str, exp: &str, tt: &str, itype: &str, aid: &str) -> String {
    let alarm = format!("{date}1200");
    let arrival = format!("{date}1206");
    rec(
        date,
        num,
        exp,
        "1005",
        tt,
        &[(8, itype), (10, aid), (11, &alarm), (12, &arrival)],
    )
}

/// The incidents, in the department's own export format (spec 2015.1).
fn transaction_file() -> String {
    let lines = [
        "^".to_string(),
        "VENDOR^SOFTWARE".to_string(),
        // 1: structure fire with an exposure; counts once.
        basic("20250105", "0000001", "000", "", "111", "N"),
        basic("20250105", "0000001", "001", "", "111", "N"),
        // 2: plus-one vehicle fire code rolls up to 131.
        basic("20250210", "0000002", "000", "", "1311", "N"),
        // 3: grass fire; Wildland acres (12.5) win over Fire-module acres (10).
        basic("20250315", "0000003", "000", "", "143", "N"),
        rec(
            "20250315",
            "0000003",
            "000",
            "1100",
            "",
            &[(11, "10"), (12, "N")],
        ),
        rec("20250315", "0000003", "000", "1300", "", &[(35, "12.5")]),
        // 4: brush fire under one acre.
        basic("20250420", "0000004", "000", "", "142", "N"),
        rec("20250420", "0000004", "000", "1100", "", &[(12, "Y")]),
        // 5: forest fire given as mutual aid: not in call volume or acreage.
        basic("20250501", "0000005", "000", "", "141", "3"),
        rec("20250501", "0000005", "000", "1300", "", &[(35, "100")]),
        // 6: MVA with automatic aid received: still primary.
        basic("20250602", "0000006", "000", "", "322", "2"),
        // 7: vehicle extrication.
        basic("20250703", "0000007", "000", "", "352", "N"),
        // 8: structure fire given as automatic aid.
        basic("20250804", "0000008", "000", "", "111", "4"),
        // 9: rescue standby.
        basic("20250905", "0000009", "000", "", "381", "N"),
        // 10: EMS call, other aid given.
        basic("20251006", "0000010", "000", "", "321", "5"),
        // 11: false alarm with mutual aid received.
        basic("20251107", "0000011", "000", "", "700", "1"),
        // 12: added, then deleted.
        basic("20251208", "0000012", "000", "", "111", "N"),
        rec("20251208", "0000012", "000", "1005", "2", &[]),
        // 13: added as good intent, changed to a false alarm.
        basic("20251209", "0000013", "000", "", "611", "N"),
        basic("20251209", "0000013", "000", "1", "735", "N"),
        // 14: no valid incident type.
        basic("20251210", "0000014", "000", "", "UUU", "N"),
        // 15: the year before.
        basic("20240115", "0000015", "000", "", "111", "N"),
    ];
    lines.join("\r\n") + "\r\n"
}

/// The same incidents as an NFIRS Public Data Release year folder (final
/// state only: no deleted or superseded records, MMDDYYYY dates).
fn write_pdr(dir: &Path) {
    let basic = [
        ("01052025", "1", "0", "111", "N"),
        ("01052025", "1", "1", "111", "N"),
        ("02102025", "2", "0", "1311", "N"),
        ("03152025", "3", "0", "143", "N"),
        ("04202025", "4", "0", "142", "N"),
        ("05012025", "5", "0", "141", "3"),
        ("06022025", "6", "0", "322", "2"),
        ("07032025", "7", "0", "352", "N"),
        ("08042025", "8", "0", "111", "4"),
        ("09052025", "9", "0", "381", "N"),
        ("10062025", "10", "0", "321", "5"),
        ("11072025", "11", "0", "700", "1"),
        ("12092025", "13", "0", "735", "N"),
        ("12102025", "14", "0", "UUU", "N"),
        ("01152024", "15", "0", "111", "N"),
    ];
    let mut s = String::from("STATE^FDID^INC_DATE^INC_NO^EXP_NO^INC_TYPE^AID^ALARM^ARRIVAL\n");
    for (d, n, e, t, aid) in basic {
        s.push_str(&format!("NY^11001^{d}^{n}^{e}^{t}^{aid}^{d}1200^{d}1206\n"));
    }
    std::fs::write(dir.join("basicincident.txt"), s).unwrap();
    std::fs::write(
        dir.join("fireincident.txt"),
        "STATE^FDID^INC_DATE^INC_NO^EXP_NO^ACRES_BURN^LESS_1ACRE\n\
         NY^11001^03152025^3^0^10^N\n\
         NY^11001^04202025^4^0^^Y\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("wildlands.txt"),
        "STATE^FDID^INC_DATE^INC_NO^EXP_NO^ACRES_BURN\n\
         NY^11001^03152025^3^0^12.5\n\
         NY^11001^05012025^5^0^100\n",
    )
    .unwrap();
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nfirs-afg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ny() -> Filter {
    Filter {
        state: Some("NY".into()),
        fdid: Some("11001".into()),
    }
}

fn expected_2025() -> AfgYear {
    AfgYear {
        year: 2025,
        // 1, 2, 3, 4, 6, 7, 9, 11, 13, 14. Not 5, 8, 10 (aid given) or 12 (deleted).
        total: 10,
        series: [4, 0, 3, 0, 0, 0, 2, 0, 0],
        unclassified: 1,
        structure_fires: 1,
        vehicle_fires: 1,
        vegetation_fires: 2,
        vegetation_acres: 12.5,
        vegetation_fires_without_acres: 0,
        motor_vehicle_accidents: 1,
        vehicle_extrications: 1,
        rescues: 1,
        mutual_aid_received: 1,
        automatic_aid_received: 1,
        mutual_aid_given: 1,
        automatic_aid_given: 1,
        other_aid_given: 1,
        aid_structure_fires: 1,
        empty_months: vec![],
    }
}

#[test]
fn transaction_file_gives_hand_checked_numbers() {
    let dir = tmpdir("txn");
    let f = dir.join("export.txt");
    std::fs::write(&f, transaction_file()).unwrap();
    let loaded = load(&[f], &Filter::default()).unwrap();
    assert_eq!(afg_year(&loaded.incidents, 2025), expected_2025());
    let y2024 = afg_year(&loaded.incidents, 2024);
    assert_eq!(
        (y2024.total, y2024.series[0], y2024.structure_fires),
        (1, 1, 1)
    );
    assert_eq!(y2024.empty_months, (2..=12).collect::<Vec<u32>>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn public_data_gives_the_same_table() {
    let dir = tmpdir("pdr");
    write_pdr(&dir);
    let loaded = load(std::slice::from_ref(&dir), &ny()).unwrap();
    assert_eq!(afg_year(&loaded.incidents, 2025), expected_2025());
    let _ = std::fs::remove_dir_all(&dir);
}

fn cli(args: &[&str]) -> (bool, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_nfirs-history"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).into(),
        String::from_utf8_lossy(&o.stderr).into(),
    )
}

#[test]
fn afg_command_prints_the_table_most_recent_year_first() {
    let dir = tmpdir("cli");
    let f = dir.join("export.txt");
    std::fs::write(&f, transaction_file()).unwrap();
    let (ok, out, _) = cli(&["afg", f.to_str().unwrap()]);
    assert!(ok);
    let header = out.lines().find(|l| l.contains("2025")).unwrap();
    assert!(header.find("2025") < header.find("2024"));
    let acres = out
        .lines()
        .find(|l| l.starts_with("Total acreage"))
        .unwrap();
    assert!(
        acres
            .trim_start_matches("Total acreage of all vegetation fires")
            .split_whitespace()
            .next()
            == Some("13"),
        "{acres}"
    );
    assert!(out.contains("EMS-BLS Response Calls"));
    assert!(out.contains("n/a"));

    let (ok, csv, _) = cli(&["afg", "--csv", "--years", "2025", f.to_str().unwrap()]);
    assert!(ok);
    assert!(csv.starts_with("category,2025\n"));
    assert!(csv.contains("\nNFIRS Series 100: Fire,4\n"));
    assert!(csv.contains("\nEMS-BLS Response Calls,\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incidents_command_writes_one_row_per_record() {
    let dir = tmpdir("inc");
    let f = dir.join("export.txt");
    std::fs::write(&f, transaction_file()).unwrap();
    let (ok, csv, _) = cli(&["incidents", "--years", "2025", f.to_str().unwrap()]);
    assert!(ok);
    // Header + 14 incidents (13 numbered, one with an exposure) in 2025.
    assert_eq!(csv.lines().count(), 1 + 14);
    assert!(csv.contains(",1311,Passenger vehicle fire,"), "{csv}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn afg_refuses_to_mix_departments() {
    let dir = tmpdir("mix");
    let f = dir.join("export.txt");
    let other = transaction_file().replace("11001^NY^20240115", "22002^NY^20240115");
    std::fs::write(&f, other).unwrap();
    let (ok, _, err) = cli(&["afg", f.to_str().unwrap()]);
    assert!(!ok);
    assert!(err.contains("more than one department"), "{err}");
    let (ok, _, _) = cli(&[
        "afg",
        "--fdid",
        "11001",
        "--state",
        "NY",
        f.to_str().unwrap(),
    ]);
    assert!(ok);
    let _ = std::fs::remove_dir_all(&dir);
}
