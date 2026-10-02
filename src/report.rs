//! The three outputs: the AFG call-volume table, the incident CSV and the
//! summary.

use crate::codes::{ACTIONS_TAKEN, AID, INCIDENT_TYPE, PROPERTY_USE, describe};
use crate::model::Incident;
use std::fmt::Write;

/// AFG application Table 7 ("Call Volume") for one calendar year, as laid
/// out in the FY2025 AFG Application Checklist (FEMA, May 2026).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AfgYear {
    pub year: i32,
    /// Incidents where the department was a primary responder.
    pub total: u64,
    /// NFIRS series 100 through 900.
    pub series: [u64; 9],
    /// Primary-responder incidents with no valid incident type.
    pub unclassified: u64,
    pub structure_fires: u64,
    pub vehicle_fires: u64,
    pub vegetation_fires: u64,
    pub vegetation_acres: f64,
    /// Vegetation fires with no acreage in the Fire or Wildland module.
    pub vegetation_fires_without_acres: u64,
    pub motor_vehicle_accidents: u64,
    pub vehicle_extrications: u64,
    pub rescues: u64,
    pub mutual_aid_received: u64,
    pub automatic_aid_received: u64,
    pub mutual_aid_given: u64,
    pub automatic_aid_given: u64,
    /// Aid code 5: counted as aid given (excluded from the call volume) but
    /// fits neither of the AFG "provide" questions.
    pub other_aid_given: u64,
    pub aid_structure_fires: u64,
    /// Months (1-12) with no incident records at all: often a missing export file.
    pub empty_months: Vec<u32>,
}

/// Count one calendar year. Each incident counts once (exposure 0 only,
/// AFG: "Each incident must be counted only once"). The call-volume rows
/// leave out aid given (codes 3, 4 and 5), per AFG: "Include only those
/// alarms which your organization was a primary responder and not second due
/// or giving mutual aid." The aid rows count every aid response.
pub fn afg_year(incidents: &[Incident], year: i32) -> AfgYear {
    let mut a = AfgYear {
        year,
        ..Default::default()
    };
    let mut seen_month = [false; 12];
    for inc in incidents
        .iter()
        .filter(|i| i.exposure == 0 && i.year() == Some(year))
    {
        if let Some(m) = inc.month() {
            seen_month[m as usize - 1] = true;
        }
        let code = inc.type_code();
        let structure = code.is_some_and(|c| (111..=123).contains(&c));
        let aid = inc.aid.trim();
        match aid {
            "1" => a.mutual_aid_received += 1,
            "2" => a.automatic_aid_received += 1,
            "3" => a.mutual_aid_given += 1,
            "4" => a.automatic_aid_given += 1,
            "5" => a.other_aid_given += 1,
            _ => {}
        }
        if structure && matches!(aid, "1" | "2" | "3" | "4") {
            a.aid_structure_fires += 1;
        }
        if inc.is_aid_given() {
            continue;
        }
        a.total += 1;
        let (Some(c), Some(s)) = (code, inc.series()) else {
            a.unclassified += 1;
            continue;
        };
        a.series[s as usize - 1] += 1;
        match c {
            111..=123 => a.structure_fires += 1,
            130..=138 => a.vehicle_fires += 1,
            140..=143 => {
                a.vegetation_fires += 1;
                match inc.acres() {
                    Some(x) => a.vegetation_acres += x,
                    None => a.vegetation_fires_without_acres += 1,
                }
            }
            322..=324 => a.motor_vehicle_accidents += 1,
            352 => a.vehicle_extrications += 1,
            300 | 351 | 353..=381 => a.rescues += 1,
            _ => {}
        }
    }
    a.empty_months = (1..=12).filter(|m| !seen_month[*m as usize - 1]).collect();
    a
}

/// The years AFG asks for: the latest year in the data and the two before it.
pub fn default_years(incidents: &[Incident]) -> Vec<i32> {
    match incidents.iter().filter_map(Incident::year).max() {
        Some(y) => vec![y, y - 1, y - 2],
        None => Vec::new(),
    }
}

const SERIES: [&str; 9] = [
    "NFIRS Series 100: Fire",
    "NFIRS Series 200: Overpressure Rupture, Explosion, Overheat (No Fire)",
    "NFIRS Series 300: Rescue & Emergency Medical Service Incident",
    "NFIRS Series 400: Hazardous Condition (No Fire)",
    "NFIRS Series 500: Service Call",
    "NFIRS Series 600: Good Intent Call",
    "NFIRS Series 700: False Alarm & False Call",
    "NFIRS Series 800: Severe Weather & Natural Disaster",
    "NFIRS Series 900: Special Incident Type",
];

const NOT_IN_NFIRS: [&str; 5] = [
    "EMS-BLS Response Calls",
    "EMS-ALS Response Calls",
    "EMS-BLS Scheduled Transports",
    "EMS-ALS Scheduled Transports",
    "Community Paramedic Response Calls",
];

/// Table rows in checklist order: label and one value per year. `None`
/// means NFIRS has no data for that row.
fn afg_rows(years: &[AfgYear]) -> Vec<(String, Vec<Option<u64>>)> {
    let row = |label: &str, f: &dyn Fn(&AfgYear) -> u64| {
        (
            label.to_string(),
            years.iter().map(|y| Some(f(y))).collect(),
        )
    };
    let mut rows = Vec::new();
    for (i, label) in SERIES.iter().enumerate() {
        rows.push(row(label, &|y| y.series[i]));
    }
    rows.push(row("Of Series 100: Structure Fire (111-123)", &|y| {
        y.structure_fires
    }));
    rows.push(row("Of Series 100: Vehicle Fire (130-138)", &|y| {
        y.vehicle_fires
    }));
    rows.push(row("Of Series 100: Vegetation Fires (140-143)", &|y| {
        y.vegetation_fires
    }));
    rows.push(row("Total acreage of all vegetation fires", &|y| {
        y.vegetation_acres.round() as u64
    }));
    rows.push(row(
        "Of Series 300: Motor Vehicle Accidents (322-324)",
        &|y| y.motor_vehicle_accidents,
    ));
    rows.push(row(
        "Of Series 300: Extrications from Vehicles (352)",
        &|y| y.vehicle_extrications,
    ));
    rows.push(row("Of Series 300: Rescues (300, 351, 353-381)", &|y| {
        y.rescues
    }));
    for label in NOT_IN_NFIRS {
        rows.push((label.to_string(), vec![None; years.len()]));
    }
    rows.push(row("Times your organization received mutual aid", &|y| {
        y.mutual_aid_received
    }));
    rows.push(row(
        "Times your organization received automatic aid",
        &|y| y.automatic_aid_received,
    ));
    rows.push(row("Times your organization provided mutual aid", &|y| {
        y.mutual_aid_given
    }));
    rows.push(row(
        "Times your organization provided automatic aid",
        &|y| y.automatic_aid_given,
    ));
    rows.push(row(
        "Of the mutual and automatic aid responses: structure fires",
        &|y| y.aid_structure_fires,
    ));
    rows
}

/// Notes a grant writer must see before copying numbers into the application.
pub fn afg_notes(years: &[AfgYear]) -> Vec<String> {
    let mut notes = vec![
        "Call-volume rows count each incident once and leave out aid given (codes 3, 4, 5), as Table 7 instructs.".to_string(),
        "EMS-BLS/ALS rows are not recorded in NFIRS; take them from your EMS records.".to_string(),
    ];
    for y in years {
        if y.unclassified > 0 {
            notes.push(format!(
                "{}: {} incident(s) have no valid incident type; they are in no series row.",
                y.year, y.unclassified
            ));
        }
        if y.vegetation_fires_without_acres > 0 {
            notes.push(format!(
                "{}: {} vegetation fire(s) have no acreage; the acreage total leaves them out.",
                y.year, y.vegetation_fires_without_acres
            ));
        }
        if y.other_aid_given > 0 {
            notes.push(format!(
                "{}: {} response(s) coded 'other aid given' (5); left out of the call volume and of both 'provided' rows.",
                y.year, y.other_aid_given
            ));
        }
        if y.empty_months.len() == 12 {
            notes.push(format!(
                "{}: no incidents in the data for this year.",
                y.year
            ));
        } else if !y.empty_months.is_empty() {
            let months: Vec<String> = y.empty_months.iter().map(u32::to_string).collect();
            notes.push(format!(
                "{}: no incidents in month(s) {}. Check that no export file is missing.",
                y.year,
                months.join(", ")
            ));
        }
    }
    notes
}

pub fn afg_text(years: &[AfgYear], department: &str) -> String {
    let rows = afg_rows(years);
    let width = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let mut s = format!("AFG Call Volume (Table 7) for {department}\n\n");
    let _ = write!(s, "{:width$}", "");
    for y in years {
        let _ = write!(s, "  {:>8}", y.year);
    }
    s.push('\n');
    for (label, values) in &rows {
        let _ = write!(s, "{label:width$}");
        for v in values {
            match v {
                Some(n) => {
                    let _ = write!(s, "  {n:>8}");
                }
                None => s.push_str("       n/a"),
            }
        }
        s.push('\n');
    }
    s.push_str("\nNotes:\n");
    for n in afg_notes(years) {
        let _ = writeln!(s, "- {n}");
    }
    s
}

pub fn afg_csv(years: &[AfgYear]) -> String {
    let mut s = String::from("category");
    for y in years {
        let _ = write!(s, ",{}", y.year);
    }
    s.push('\n');
    for (label, values) in afg_rows(years) {
        s.push_str(&csv_field(&label));
        for v in values {
            s.push(',');
            if let Some(n) = v {
                s.push_str(&n.to_string());
            }
        }
        s.push('\n');
    }
    s
}

/// Quote a CSV field when needed, and defuse text that a spreadsheet would
/// run as a formula (the inputs come from other people's software).
pub fn csv_field(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

const INCIDENT_COLUMNS: &[&str] = &[
    "date",
    "incident_number",
    "exposure",
    "station",
    "incident_type",
    "incident_type_description",
    "aid",
    "aid_description",
    "alarm",
    "arrival",
    "controlled",
    "last_unit_cleared",
    "response_minutes",
    "address",
    "city",
    "zip",
    "actions_taken",
    "property_use",
    "property_use_description",
    "property_loss",
    "contents_loss",
    "fire_service_deaths",
    "other_deaths",
    "fire_service_injuries",
    "other_injuries",
    "acres_burned",
    "state",
    "fdid",
];

/// One row per incident record (exposures included), in input order.
pub fn incidents_csv(incidents: &[Incident]) -> String {
    let opt = |v: Option<i64>| v.map(|n| n.to_string()).unwrap_or_default();
    let dt = |v: Option<crate::model::DateTime>| v.map(|d| d.to_string()).unwrap_or_default();
    let mut s = INCIDENT_COLUMNS.join(",");
    s.push('\n');
    for i in incidents {
        let actions: Vec<String> = i
            .actions
            .iter()
            .map(|a| match describe(ACTIONS_TAKEN, a) {
                "" => a.clone(),
                d => format!("{a} {d}"),
            })
            .collect();
        let fields = [
            i.date.map(|d| d.to_string()).unwrap_or_default(),
            i.number.clone(),
            i.exposure.to_string(),
            i.station.clone(),
            i.incident_type.clone(),
            describe(INCIDENT_TYPE, &i.incident_type).to_string(),
            i.aid.clone(),
            describe(AID, &i.aid).to_string(),
            dt(i.alarm),
            dt(i.arrival),
            dt(i.controlled),
            dt(i.cleared),
            i.response_minutes()
                .map(|m| m.to_string())
                .unwrap_or_default(),
            i.address.clone(),
            i.city.clone(),
            i.zip.clone(),
            actions.join("; "),
            i.property_use.clone(),
            describe(PROPERTY_USE, &i.property_use).to_string(),
            opt(i.property_loss),
            opt(i.contents_loss),
            opt(i.ff_deaths),
            opt(i.other_deaths),
            opt(i.ff_injuries),
            opt(i.other_injuries),
            i.acres().map(|a| a.to_string()).unwrap_or_default(),
            i.state.clone(),
            i.fdid.clone(),
        ];
        let row: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
        s.push_str(&row.join(","));
        s.push('\n');
    }
    s
}

/// Nearest-rank percentile of a sorted, non-empty slice.
fn percentile(sorted: &[i64], p: f64) -> i64 {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// Town-report summary for one year: counts by series and month, and
/// response times for the department's own (not aid-given) responses.
pub fn summary_text(incidents: &[Incident], year: i32) -> String {
    let year_incidents: Vec<&Incident> = incidents
        .iter()
        .filter(|i| i.exposure == 0 && i.year() == Some(year))
        .collect();
    let mut s = format!("{year}: {} incidents\n", year_incidents.len());
    let mut series = [0u64; 9];
    let mut unclassified = 0;
    let mut months = [0u64; 12];
    let mut aid_given = 0;
    let mut minutes = Vec::new();
    for i in &year_incidents {
        match i.series() {
            Some(n) => series[n as usize - 1] += 1,
            None => unclassified += 1,
        }
        if let Some(m) = i.month() {
            months[m as usize - 1] += 1;
        }
        if i.is_aid_given() {
            aid_given += 1;
        } else if let Some(m) = i.response_minutes() {
            minutes.push(m);
        }
    }
    s.push_str("\nBy type:\n");
    for (label, n) in SERIES.iter().zip(series) {
        let _ = writeln!(s, "  {n:>6}  {}", label.trim_start_matches("NFIRS "));
    }
    if unclassified > 0 {
        let _ = writeln!(s, "  {unclassified:>6}  No valid incident type");
    }
    let _ = writeln!(
        s,
        "  ({aid_given} of these were aid given to other departments)"
    );
    s.push_str("\nBy month:\n");
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    for (name, n) in NAMES.iter().zip(months) {
        let _ = writeln!(s, "  {name}  {n:>6}");
    }
    minutes.sort_unstable();
    s.push_str("\nResponse time, alarm to first arrival (own responses):\n");
    if minutes.is_empty() {
        s.push_str("  no incidents with both alarm and arrival times\n");
    } else {
        let _ = writeln!(
            s,
            "  median {} min, 90th percentile {} min ({} incidents with times)",
            percentile(&minutes, 50.0),
            percentile(&minutes, 90.0),
            minutes.len()
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quoting_and_formula_defusing() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=HYPERLINK(1)"), "'=HYPERLINK(1)");
        assert_eq!(csv_field("-5"), "-5");
    }

    #[test]
    fn nearest_rank_percentile() {
        let v = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(percentile(&v, 50.0), 5);
        assert_eq!(percentile(&v, 90.0), 9);
        assert_eq!(percentile(&[7], 90.0), 7);
    }
}
