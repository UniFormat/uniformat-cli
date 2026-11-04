//! Command line of the UniFormat family: `formats`, `info`, `schema`, `count`, `csv`, `convert`.
//!
//! The CLI only talks to the common interface of `uniformat-core` (registry, `Format`, Arrow batches): it does not
//! know any format by name. The formats it can handle are the ones linked at build time (Cargo features).
//! Exit codes: 0 success, 1 read/write error, 2 usage error.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use serde_json::{json, Map, Value};
use uniformat_core::{convert, Error, ReadRequest, Registry, WriteRequest};

/// The registry of the formats linked into this binary.
#[must_use]
pub fn default_registry() -> Registry {
    #[allow(unused_mut)]
    let mut r = Registry::new();
    #[cfg(feature = "sas")]
    uniformat_sas_arrow::register(&mut r);
    r
}

#[derive(Debug)]
struct CliError {
    message: String,
    code: u8,
}

impl From<Error> for CliError {
    fn from(e: Error) -> Self {
        let code = if matches!(e, Error::InvalidOption { .. } | Error::UnknownFormat(_)) {
            2
        } else {
            1
        };
        Self {
            message: e.to_string(),
            code,
        }
    }
}

impl From<io::Error> for CliError {
    fn from(e: io::Error) -> Self {
        Self {
            message: e.to_string(),
            code: 1,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "uniformat",
    version,
    about = "Read, inspect and convert tabular files (UniFormat)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List the formats linked into this binary.
    Formats,
    /// Table information: schema, metadata, row count (no data is read when the format allows it).
    Info(InfoArgs),
    /// Arrow schema with the metadata of every field.
    Schema(Input),
    /// Number of rows.
    Count(Input),
    /// Export the data as CSV.
    Csv(CsvArgs),
    /// Convert a file to another format through the common data model.
    Convert(ConvertArgs),
}

#[derive(Args, Debug)]
struct Input {
    /// Input file.
    file: PathBuf,
    /// Format name (default: chosen from the file extension).
    #[arg(long)]
    format: Option<String>,
    /// Format-specific read option `key=value` (repeatable), for example `missing=sidecar`.
    #[arg(long = "read-opt", value_name = "KEY=VALUE")]
    read_opts: Vec<String>,
}

#[derive(Args, Debug)]
struct InfoArgs {
    #[command(flatten)]
    input: Input,
    /// JSON output.
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct CsvArgs {
    #[command(flatten)]
    input: Input,
    /// Columns to export, comma separated.
    #[arg(long, value_delimiter = ',')]
    columns: Option<Vec<String>>,
    /// Maximum number of rows.
    #[arg(long)]
    limit: Option<u64>,
    /// Output file (default: standard output).
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ConvertArgs {
    /// Input file.
    src: PathBuf,
    /// Output file.
    dst: PathBuf,
    /// Input format name (default: from the extension).
    #[arg(long)]
    from: Option<String>,
    /// Output format name (default: from the extension).
    #[arg(long)]
    to: Option<String>,
    /// Columns to keep, comma separated.
    #[arg(long, value_delimiter = ',')]
    columns: Option<Vec<String>>,
    /// Maximum number of rows.
    #[arg(long)]
    limit: Option<u64>,
    /// Read option `key=value` (repeatable).
    #[arg(long = "read-opt", value_name = "KEY=VALUE")]
    read_opts: Vec<String>,
    /// Write option `key=value` (repeatable), for example `compression=char`.
    #[arg(long = "write-opt", value_name = "KEY=VALUE")]
    write_opts: Vec<String>,
}

fn kv(items: &[String]) -> Result<BTreeMap<String, String>, CliError> {
    items
        .iter()
        .map(|s| {
            s.split_once('=')
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .ok_or_else(|| CliError {
                    message: format!("option {s:?}: expected key=value"),
                    code: 2,
                })
        })
        .collect()
}

fn read_request(
    opts: &[String],
    columns: Option<Vec<String>>,
    limit: Option<u64>,
) -> Result<ReadRequest, CliError> {
    Ok(ReadRequest {
        columns,
        limit,
        options: kv(opts)?,
    })
}

fn pick(
    registry: &Registry,
    path: &std::path::Path,
    name: Option<&str>,
) -> Result<std::sync::Arc<dyn uniformat_core::Format>, CliError> {
    Ok(match name {
        Some(n) => registry.by_name(n)?,
        None => registry.by_path(path)?,
    })
}

fn field_json(i: usize, f: &arrow_schema::Field) -> Value {
    let mut meta = Map::new();
    for (k, v) in f.metadata() {
        meta.insert(k.clone(), Value::String(v.clone()));
    }
    json!({ "index": i, "name": f.name(), "type": f.data_type().to_string(), "nullable": f.is_nullable(), "metadata": Value::Object(meta) })
}

fn run_command(cmd: &Command, registry: &Registry, out: &mut dyn Write) -> Result<(), CliError> {
    match cmd {
        Command::Formats => {
            for f in registry.formats() {
                writeln!(
                    out,
                    "{:<12} .{:<14} {}",
                    f.name(),
                    f.extensions().join(",."),
                    if f.can_write() { "read, write" } else { "read" }
                )?;
            }
        }
        Command::Info(a) => {
            let f = pick(registry, &a.input.file, a.input.format.as_deref())?;
            let info = f.info(
                &a.input.file,
                &read_request(&a.input.read_opts, None, None)?,
            )?;
            let mut table = Map::new();
            for (k, v) in info.schema.metadata() {
                table.insert(k.clone(), Value::String(v.clone()));
            }
            let fields: Vec<Value> = info
                .schema
                .fields()
                .iter()
                .enumerate()
                .map(|(i, f)| field_json(i, f))
                .collect();
            let doc = json!({ "format": f.name(), "rows": info.row_count, "table": Value::Object(table), "columns": fields });
            if a.json {
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| CliError {
                        message: e.to_string(),
                        code: 1
                    })?
                )?;
            } else {
                writeln!(out, "format {}", f.name())?;
                match info.row_count {
                    Some(n) => writeln!(out, "rows {n}")?,
                    None => writeln!(out, "rows unknown without reading the data")?,
                }
                for (k, v) in info.schema.metadata().iter().collect::<BTreeMap<_, _>>() {
                    writeln!(out, "{k:<24} {v}")?;
                }
                writeln!(out)?;
                for (i, fld) in info.schema.fields().iter().enumerate() {
                    let label = fld
                        .metadata()
                        .iter()
                        .find(|(k, _)| k.ends_with(".label"))
                        .map_or("", |(_, v)| v.as_str());
                    writeln!(
                        out,
                        "{i:<4} {:<32} {:<26} {label}",
                        fld.name(),
                        fld.data_type().to_string()
                    )?;
                }
            }
        }
        Command::Schema(a) => {
            let f = pick(registry, &a.file, a.format.as_deref())?;
            let info = f.info(&a.file, &read_request(&a.read_opts, None, None)?)?;
            for fld in info.schema.fields() {
                writeln!(out, "{}: {}", fld.name(), fld.data_type())?;
                for (k, v) in fld.metadata().iter().collect::<BTreeMap<_, _>>() {
                    writeln!(out, "    {k} = {v}")?;
                }
            }
        }
        Command::Count(a) => {
            let f = pick(registry, &a.file, a.format.as_deref())?;
            let req = read_request(&a.read_opts, None, None)?;
            let n = match f.info(&a.file, &req)?.row_count {
                Some(n) => n,
                None => {
                    let mut total = 0u64;
                    for b in f.read(&a.file, &req)? {
                        total += b.map_err(Error::from)?.num_rows() as u64;
                    }
                    total
                }
            };
            writeln!(out, "{n}")?;
        }
        Command::Csv(a) => {
            let f = pick(registry, &a.input.file, a.input.format.as_deref())?;
            let stream = f.read(
                &a.input.file,
                &read_request(&a.input.read_opts, a.columns.clone(), a.limit)?,
            )?;
            let sink: Box<dyn Write + '_> = match &a.output {
                Some(p) => Box::new(BufWriter::new(File::create(p)?)),
                None => Box::new(out),
            };
            let mut w = arrow_csv::WriterBuilder::new()
                .with_header(true)
                .build(sink);
            for b in stream {
                w.write(&b.map_err(Error::from)?).map_err(Error::from)?;
            }
        }
        Command::Convert(a) => {
            let read = read_request(&a.read_opts, a.columns.clone(), a.limit)?;
            let write = WriteRequest {
                options: kv(&a.write_opts)?,
            };
            let summary = convert(
                registry,
                &a.src,
                &a.dst,
                a.from.as_deref(),
                a.to.as_deref(),
                &read,
                &write,
            )?;
            writeln!(out, "{} rows written to {}", summary.rows, a.dst.display())?;
            for w in &summary.warnings {
                // warnings go to the standard output of the command only through the summary line count
                writeln!(out, "warning: {w}")?;
            }
        }
    }
    Ok(())
}

/// Runs the command line with the given streams and registry; returns the exit code.
pub fn run_with<I, T>(args: I, registry: &Registry, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            let text = e.render().to_string();
            let sink: &mut dyn Write = if code == 0 { out } else { err };
            let _ = write!(sink, "{text}");
            return code;
        }
    };
    match run_command(&cli.command, registry, out) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "uniformat: {}", e.message);
            e.code
        }
    }
}

/// Entry point: standard streams and the registry of the linked formats.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut err = io::stderr().lock();
    let code = run_with(args, &default_registry(), &mut out, &mut err);
    let _ = out.flush();
    ExitCode::from(code)
}
