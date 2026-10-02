---
slug: nfirs-history
title: CLI that turns a fire department's NFIRS export files or public-data records into a readable incident CSV and the AFG call-volume table
verdict: build
---

## Problem

NFIRS, the federal fire incident reporting system, shut down on 31 January 2026
and was replaced by NERIS. NERIS does not import NFIRS history. Two things went
away with NFIRS:

1. **The department's own copy of its records.** Many small and volunteer
   departments entered incidents straight into the free federal eNFIRS web tool
   and used it as their records system. State fire marshals told them to pull
   their data out before the deadline or lose it. Illinois OSFM wrote: "Once the
   system is decommissioned, the incidents will be permanently deleted"
   (https://sfm.illinois.gov/iam/firedepartment/nfirs.html). The only way out
   was the Bulk Export Utility. It gives one ZIP per year that holds an NFIRS 5.0
   *transaction file*: caret-delimited, about 30 record types mixed in one file,
   coded values. Responserack's guide to that export says: "these exported files
   are not human-readable … the data will need to be converted"
   (https://www.responserack.com/posts/exporting-your-nfirs-data/).
   Responserack offers to do the conversion for its customers.
2. **The grant numbers.** The FEMA Assistance to Firefighters Grant (AFG) asks
   for a "Call Volume" table: responses per year for the last three calendar
   years by NFIRS series 100–900, structure, vehicle and vegetation fires (by
   code range), vegetation-fire acreage, motor-vehicle accidents, extrications,
   rescues, and mutual/automatic aid given and received (FY2022, FY2023 and
   May 2026 FY2025 AFG application checklists, Table 7). Departments used to
   get this from the "AFG Summary Report" in the NFIRS Data Warehouse. Illinois
   OSFM told departments to run that report before the shutdown
   (https://sfm.illinois.gov/content/dam/soi/en/web/sfm/sfmdocuments/documents/AFGreportDatawarehouse.pdf).
   The Data Warehouse is gone. The FY2025 checklist (May 2026) still asks for
   NFIRS series counts for 2023–2025, and the FY2026 cycle will still need
   2024–2025 from NFIRS.

Departments that missed the export deadline have only one source left: the
annual NFIRS Public Data Release (PDR) on OpenFEMA. USFA says the PDR needs "a
database management system and expertise in SQL"
(https://www.usfa.fema.gov/data-insights/nfirs/data/). A year is 12–15 GB
uncompressed and has millions of rows in `basicincident.txt`, more than Excel
can open.

## Who benefits

Volunteer and small combination fire departments in the US: the chief,
secretary or grant writer who needs (a) a readable list of past incidents for
records retention, town reports or legal requests and (b) the AFG call-volume
numbers. There are roughly 18,000 volunteer departments, and AFG is their main
federal equipment fund. They would find the tool through a search for "NFIRS
export convert" or "AFG call volume NFIRS", through state fire marshal NERIS
pages, or through grant-writer forums. They would run a single downloaded
binary on a Windows or Mac laptop against the ZIP folder they already
exported, or against the PDR text files for their state and FDID.

## Existing solutions

- **NFIRS Data Warehouse AFG Summary Report**: decommissioned February 2026.
- **eNFIRS reports**: decommissioned with NFIRS.
- **Responserack** (https://www.responserack.com): a commercial volunteer-department
  RMS. It converts exports for customers and "in a pinch" for others. This is a
  paid or favour-based service, not a tool.
- **Commercial RMS vendors** (ESO, First Due, ImageTrend, Emergency Reporting):
  history stays with departments that already paid for them. eNFIRS-only
  departments were never on them.
- **FEMA/nfirs-database-import** (https://github.com/FEMA/nfirs-database-import,
  pushed 2025-10): SQL scripts that rebuild a full PDR year in
  SQLite/DuckDB/Postgres. It is for analysts. It needs SQL skills and about
  12 GB per year, and it computes nothing for a department.
- **dnchelst/NFIRS** (R scripts, last push 2022) and **jaflores10/nfirs-data-pipeline**
  (2024): national analysis pipelines for researchers.
- **rodneygauna/emberstone**: a FOSS NFIRS-style RMS web app. It does not read
  exports.
- **bayscott/NFIRSViewer**: listed in search, but the repository returns 404.
- **Excel import with `^` delimiter**: the transaction file mixes record types
  with different column layouts in one file, so columns do not line up. PDR
  files exceed Excel's row limit.
- **USFA "U.S. Fire Department Responses" ArcGIS layer**: a simplified 2019–2025
  map layer. It is web-only, needs ArcGIS skills to filter and export, and
  lacks the aid and acreage fields.
- crates.io: nothing for NFIRS.

## Why build anything

No free tool reads either the department's own NFIRS 5.0 transaction export or
the PDR and produces what the department needs. The official tool that did it
was switched off. The remaining options are a paid vendor, SQL skills, or
hand-counting thousands of incidents. The gap is narrow and well defined. The
file format is a published federal spec (NFIRS 5.0 Design Documentation 2015.1,
"Incident Flat File Transfer Format"). The output format is fixed by the AFG
application.

## Smallest useful intervention

`nfirs-history`, one offline binary with three commands:

- `afg`: print the AFG Table 7 call-volume numbers per calendar year (text, or
  CSV with `--csv`). It uses the code ranges from the AFG checklist, counts
  each incident once (exposure 0), and leaves aid-given responses out of the
  call volume, as AFG instructs.
- `incidents`: write one readable CSV row per incident: date, number, station,
  incident type code and description, aid, alarm/arrival/controlled/cleared
  times, response minutes, address, actions taken, property use, losses,
  casualties, acres. It leaves out names, patient data and remarks.
- `summary`: print counts by incident series and by month, with response-time
  median and 90th percentile, for town reports.

Inputs: NFIRS 5.0 transaction files (the Bulk Export Utility contents, or any
vendor's NFIRS 5.0 export), or a PDR year directory (`basicincident.txt`,
`incidentaddress.txt`, `fireincident.txt`, `wildlands.txt`) filtered with
`--state` and `--fdid`. It streams the files, so a multi-GB PDR file runs in
constant memory.

## Success criterion

- Test fixtures written to the published spec give exact, hand-checked AFG
  numbers. This includes the edge cases: exposures, aid given, plus-one codes,
  vegetation-fire acreage taken from the wildland or fire module, and
  change/delete transactions.
- A PDR fixture and a transaction-file fixture holding the same incidents give
  identical AFG tables.
- A department can go from an unzipped export folder to the three-year AFG
  table with one command and no install beyond the binary.
- Filtering one FDID out of a full-size `basicincident.txt` (several GB) uses
  constant memory and finishes in minutes on a laptop.

## Maintenance

Low. NFIRS is frozen: the spec will not change again, and the 1980–2025 PDR is
an archive. AFG will move to NERIS categories for years after 2025. The tool
stays useful as long as any year in the AFG three-year window is an NFIRS year
(through about the FY2027 cycle), and for records lookups indefinitely. Things
that could break it: OpenFEMA renaming PDR columns (the tool reads columns by
header name, so reordering is harmless), or state-specific extensions (7000 and
8000 series records are skipped). No server, no network, no keys.

## Decision

**Build.** The need is concrete and dated, and it has sources: state fire
marshals and a vendor describe the unreadable export, and FEMA's current AFG
application asks for NFIRS series counts the Data Warehouse used to produce.
The free official path was shut down. The alternatives are paid services or
SQL work on 12 GB datasets, which volunteer chiefs cannot do. The format is
fully specified and frozen, so a small, testable, offline tool covers the gap.
It does not give advice. It only counts and decodes the department's own
records.

**Build status (review, unattended):** the verdict still stands, but the
build is unfinished. Only the input readers (`src/txn.rs`, `src/pdr.rs`,
`src/input.rs`) and code tables exist. The `afg`, `incidents` and `summary`
commands, the fixtures and the success criteria above are not implemented
or met. The AFG rule "leave aid-given responses out of the call volume" is
not backed by any cited source and must be checked against the AFG
checklist before `afg` is written. Code ranges confirmed from FEMA checklist
text: structure 111-123, vehicle 130-138, vegetation 140-143, motor-vehicle
accidents 322-324, vehicle extrications 352, rescues 300, 351, 353-381.

## Also considered

- inat-species-timeseries (species counts per year from an iNaturalist project, forum.inaturalist.org/t/62814): forum replies show export plus Excel or the API already covers it, and the asker was told it is routine.
- council-agenda-keyword-alerts (alerts when a city agenda mentions a topic): Civic Band (civic.band, 1,000+ municipalities searchable), city Legistar and Granicus subscriptions and several Apify actors already do it.
- drive-public-link-audit (find "anyone with the link" Google Drive files in a small nonprofit): alulsh/drive-public-files (Apps Script), Workspace admin audit and many free scanners cover it.
- lead-service-line-inventory-check (validate small water systems' LCRR inventories): every state and EPA provide Excel templates with built-in validation and TA providers, and I found no sourced unmet need.
