# nfirs-history

**Status: incomplete. Not usable by fire departments yet.**

Goal (see [PROBLEM.md](PROBLEM.md)): read a fire department's NFIRS 5.0
Bulk Export transaction files, or the NFIRS Public Data Release for one
state/FDID, and produce a readable incident CSV, a summary, and the AFG
call-volume table.

What exists today:

- A reader for NFIRS 5.0 transaction files (spec 2015.1, "Incident Flat File
  Transfer Format"): record types 1000, 1005, 1010, 1100 and 1300, with
  add/change/delete/no-activity transactions. Plain files, folders and ZIPs.
- A streaming reader for Public Data Release year folders (`basicincident.txt`,
  `incidentaddress.txt`, `fireincident.txt`, `wildlands.txt`). It refuses to
  run without both `--state` and `--fdid` filters.
- NFIRS code tables (incident type, aid, actions taken, property use).

What does **not** exist yet: the `afg`, `incidents` and `summary` commands.
The binary only prints a "not implemented" message and exits with status 2.
Do not use anything from this repository for a grant application.

## Build

```
git clone https://github.com/SignalForHumanity/nfirs-history
cd nfirs-history
cargo test
```

## Privacy

Incident records contain street addresses. For EMS calls an address plus a
date can identify a patient. Treat any output as the department's
confidential records, not as something to publish.

## License

MIT OR Apache-2.0.
