# uniformat-cli

The `uniformat` command line: inspect, count, export and convert tabular files through the UniFormat common data model
(Apache Arrow). It only uses the interfaces of [`uniformat-core`](https://github.com/UniFormat/uniformat-core); the
formats it understands are the ones linked at build time (Cargo features, `sas` by default).

```bash
uniformat formats                                   # linked formats
uniformat info data.sas7bdat [--json]               # schema, metadata, row count (no data read when possible)
uniformat schema data.sas7bdat --read-opt missing=sidecar
uniformat count data.sas7bdat
uniformat csv data.sas7bdat --columns x,y --limit 100 -o out.csv
uniformat convert data.sas7bdat copy.sas7bdat --write-opt compression=binary
```

Options are `key=value` pairs defined by each format (`--read-opt`, `--write-opt`). Exit codes: 0 success, 1 read or write
error, 2 usage error. Writes are transactional.

## Build and test

Sibling repositories are expected side by side (`../uniformat-core`, `../uniformat-sas`, `../uniformat-conformance`).

```bash
cargo build --release          # binary: target/release/uniformat
UNIFORMAT_CORPUS=<corpus> cargo test
```

Status: 0.1, not published. License `MIT OR Apache-2.0` (provisional).
