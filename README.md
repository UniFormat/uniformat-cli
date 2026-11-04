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

## Security checks

`cargo test --locked` runs a synthetic conversion regression without a corpus: it checks private-file permissions and that a resource-limit failure leaves the destination unchanged. External corpus checks are explicitly ignored and require `UNIFORMAT_CORPUS=... cargo test --locked -- --ignored`.

SAS reads have bounded output columns and batch buffers. Use `--read-opt max_batch_bytes=N` or `--read-opt max_output_columns=N` to choose a different trusted-input budget. Writes create private temporary files; new Unix files are 0600 and replacements preserve ordinary permission bits. Destination directories must be trusted; extended ACLs are not copied. Run `cargo deny --locked --all-features check advisories` before publishing; CI pins both actions and sibling commits and audits development dependencies too.
