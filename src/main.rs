//! The `afg`, `incidents` and `summary` commands described in PROBLEM.md are
//! not implemented yet. Say so, instead of pretending to work.

fn main() {
    eprintln!(
        "nfirs-history {}: the afg, incidents and summary commands are not implemented yet.\n\
         Only the NFIRS 5.0 transaction-file and public-data readers exist (library code).\n\
         See README.md.",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2);
}
