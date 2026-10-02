# nfirs-history

NFIRS shut down on 31 January 2026, and with it the free reports many small
and volunteer fire departments relied on: their incident list in eNFIRS and
the Data Warehouse "AFG Summary Report". `nfirs-history` rebuilds both,
offline, from the files that are left. See [PROBLEM.md](PROBLEM.md) for the
background and sources.

## What it reads

- **Your department's NFIRS export**: the NFIRS 5.0 transaction files from
  the Bulk Export Utility, or from any vendor's NFIRS 5.0 export. Point it at
  the files, the ZIPs, or the folder holding them.
- **The NFIRS Public Data Release** from OpenFEMA, if you missed the export
  deadline: an unzipped year folder (`basicincident.txt`,
  `incidentaddress.txt`, `fireincident.txt`, `wildlands.txt`). This holds
  every department in the country, so `--state` and `--fdid` are required.
  Large files are streamed, not loaded into memory.

## Commands

```
nfirs-history afg       [options] <files or folders>   AFG Table 7 call volume
nfirs-history incidents [options] <files or folders>   one CSV row per incident
nfirs-history summary   [options] <files or folders>   counts by type and month, response times

--state XX --fdid 12345   pick one department
--years 2023,2024,2025    years to report
--csv                     (afg) CSV instead of a text table
-o FILE                   write to FILE
```

Examples:

```
nfirs-history afg ~/NFIRS-export/
nfirs-history afg --state OH --fdid 12345 ~/Downloads/NFIRS_2023 ~/Downloads/NFIRS_2024 ~/Downloads/NFIRS_2025
nfirs-history incidents ~/NFIRS-export/ -o incidents.csv
```

## How the AFG numbers are counted

The `afg` table follows Table 7 ("Call Volume") of FEMA's FY2025 AFG
Application Checklist (May 2026), row for row, most recent year first.

- Each incident counts once: exposures are not counted again.
- The call-volume rows leave out aid given to other departments (aid codes
  3, 4 and 5). The checklist says: "Include only those alarms which your
  organization was a primary responder and not second due or giving mutual
  aid." Aid received (codes 1 and 2) is counted.
- Code ranges are the checklist's: structure fires 111-123, vehicle fires
  130-138, vegetation fires 140-143, motor vehicle accidents 322-324,
  extrications from vehicles 352, rescues 300, 351 and 353-381. Plus-one
  codes (a fourth local digit) count under their three-digit code.
- Vegetation-fire acreage uses the Wildland module's total acres when
  present, otherwise the Fire module's acres ("less than one acre" counts as
  0), rounded to a whole number.
- The aid rows count every aid response, including aid given.
- The EMS-BLS/ALS rows are not recorded in NFIRS and show `n/a`. Take them
  from your EMS records.

Read the notes printed under the table before copying numbers into an
application. They flag incidents with no type, vegetation fires with no
acreage, "other aid given" responses, and months with no incidents (often a
missing export file).

## Not covered

- Public Data Release years before 2012 (dBASE format).
- State-specific record types (7000 and 8000 series) are skipped.
- NERIS data (2025 onward in some states, 2026 onward everywhere).

## Build

```
cargo install --git https://github.com/SignalForHumanity/nfirs-history
```

## Privacy

Incident records contain street addresses, and for EMS calls an address
plus a date can identify a patient. The `incidents` CSV is your department's
confidential record; do not publish it. The tool reads no names, patient
data or narrative remarks and makes no network connections.

## License

MIT OR Apache-2.0.
