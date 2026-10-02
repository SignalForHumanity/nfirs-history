use nfirs_history::input::load;
use nfirs_history::model::Incident;
use nfirs_history::report;
use nfirs_history::txn::Filter;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: nfirs-history <command> [options] <file-or-folder>...

commands:
  afg         AFG application call-volume table (Table 7)
  incidents   one CSV row per incident
  summary     counts by type and month, response times

inputs: NFIRS 5.0 transaction files from the Bulk Export Utility (plain,
zipped, or folders of them), or an NFIRS Public Data Release year folder.

options:
  --state XX       department state (required for public data)
  --fdid 12345     department FDID (required for public data)
  --years Y,Y,...  years to report (afg default: latest year in the data and
                   the two before it; others default: every year)
  --csv            afg: write CSV instead of a text table
  -o FILE          write output to FILE instead of the screen";

struct Args {
    command: String,
    inputs: Vec<PathBuf>,
    filter: Filter,
    years: Option<Vec<i32>>,
    csv: bool,
    output: Option<PathBuf>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let command = args.next().ok_or("no command given")?;
    if !matches!(command.as_str(), "afg" | "incidents" | "summary") {
        return Err(format!("unknown command '{command}'"));
    }
    let mut a = Args {
        command,
        inputs: Vec::new(),
        filter: Filter::default(),
        years: None,
        csv: false,
        output: None,
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--state" => a.filter.state = Some(value("--state")?),
            "--fdid" => a.filter.fdid = Some(value("--fdid")?),
            "--years" => {
                let years = value("--years")?
                    .split(',')
                    .map(|y| y.trim().parse::<i32>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "--years takes years like 2023,2024,2025".to_string())?;
                a.years = Some(years);
            }
            "--csv" => a.csv = true,
            "-o" => a.output = Some(PathBuf::from(value("-o")?)),
            s if s.starts_with('-') => return Err(format!("unknown option '{s}'")),
            _ => a.inputs.push(PathBuf::from(arg)),
        }
    }
    if a.inputs.is_empty() {
        return Err("no input files or folders given".into());
    }
    Ok(a)
}

/// Every (state, FDID) in the data; AFG and summary numbers must be for one department.
fn departments(incidents: &[Incident]) -> BTreeSet<String> {
    incidents
        .iter()
        .map(|i| {
            format!(
                "{} {}",
                i.state.trim().to_ascii_uppercase(),
                i.fdid.trim().trim_start_matches('0')
            )
        })
        .collect()
}

fn run(a: Args) -> Result<String, String> {
    let loaded = load(&a.inputs, &a.filter).map_err(|e| e.to_string())?;
    for w in &loaded.warnings {
        eprintln!("warning: {w}");
    }
    let depts = departments(&loaded.incidents);
    eprintln!(
        "read {} input(s): {} incident records, {} department(s)",
        loaded.files_read,
        loaded.incidents.len(),
        depts.len()
    );
    if loaded.incidents.is_empty() {
        return Err("no incidents found in the inputs".into());
    }
    if a.command != "incidents" && depts.len() > 1 {
        let list: Vec<&str> = depts.iter().map(String::as_str).collect();
        return Err(format!(
            "the inputs hold more than one department ({}); pick one with --state and --fdid",
            list.join(", ")
        ));
    }
    let department = depts.into_iter().next().unwrap_or_default();
    let in_years = |i: &Incident| {
        a.years
            .as_ref()
            .is_none_or(|ys| i.year().is_some_and(|y| ys.contains(&y)))
    };
    Ok(match a.command.as_str() {
        "afg" => {
            let years = a
                .years
                .clone()
                .unwrap_or_else(|| report::default_years(&loaded.incidents));
            let table: Vec<_> = years
                .iter()
                .map(|y| report::afg_year(&loaded.incidents, *y))
                .collect();
            if a.csv {
                for n in report::afg_notes(&table) {
                    eprintln!("note: {n}");
                }
                report::afg_csv(&table)
            } else {
                report::afg_text(&table, &department)
            }
        }
        "incidents" => {
            let rows: Vec<Incident> = loaded.incidents.into_iter().filter(in_years).collect();
            report::incidents_csv(&rows)
        }
        _ => {
            let mut years: Vec<i32> = loaded.incidents.iter().filter_map(Incident::year).collect();
            years.sort_unstable();
            years.dedup();
            years.retain(|y| a.years.as_ref().is_none_or(|ys| ys.contains(y)));
            let parts: Vec<String> = years
                .iter()
                .map(|y| report::summary_text(&loaded.incidents, *y))
                .collect();
            format!("{department}\n\n{}", parts.join("\n"))
        }
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().is_none_or(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let output = a.output.clone();
    match run(a) {
        Ok(text) => match output {
            Some(path) => match std::fs::write(&path, text) {
                Ok(()) => {
                    eprintln!("wrote {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {}: {e}", path.display());
                    ExitCode::FAILURE
                }
            },
            None => {
                print!("{text}");
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
