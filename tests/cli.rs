//! Command line: usage, and (with the corpus) info / count / csv / convert through the registry.

use std::process::Command;

use uniformat_testkit::{values_equal, Corpus, ReferenceTable};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_uniformat"))
}

#[test]
fn usage_and_errors() {
    assert_eq!(bin().output().unwrap().status.code(), Some(2));
    assert_eq!(
        bin().arg("--version").output().unwrap().status.code(),
        Some(0)
    );
    let out = bin().args(["count", "missing.sas7bdat"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = bin().args(["count", "file.unknownext"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = bin()
        .args(["count", "x.sas7bdat", "--read-opt", "novalue"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = bin().arg("formats").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("sas7bdat"));
}

#[test]
fn info_count_csv_and_convert_on_the_corpus() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS not set: test skipped");
        return;
    };
    let file = corpus.root().join("files/taille/taille_n50000_no.sas7bdat");
    let count = bin().arg("count").arg(&file).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&count.stdout).trim(), "50000");
    let info = bin().args(["info", "--json"]).arg(&file).output().unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(doc["format"], "sas7bdat");
    assert_eq!(doc["rows"], 50000);
    assert_eq!(doc["table"]["sas.row_count"], "50000");
    // CSV equals the reference CSV of the corpus (raw SAS dates)
    let reference = ReferenceTable::parse_bytes(
        &std::fs::read(corpus.root().join("files/taille/taille_n50000_no.csv")).unwrap(),
    )
    .unwrap();
    let out = bin()
        .args(["csv", "--limit", "200", "--read-opt", "dates=float"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got = ReferenceTable::parse_bytes(&out.stdout).unwrap();
    assert_eq!(got.row_count(), 200);
    for name in got.columns().to_vec() {
        for i in 0..200 {
            assert!(
                values_equal(
                    reference.cell(i, &name).unwrap(),
                    got.cell(i, &name).unwrap()
                ),
                "row {i} column {name}"
            );
        }
    }
    // sas -> sas, plain to compressed, through the common model; special missing codes survive
    let miss = corpus
        .root()
        .join("files/valeurs/valeurs_manquants_no.sas7bdat");
    let dir = std::env::temp_dir().join(format!("uniformat-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dst = dir.join("copy.sas7bdat");
    let conv = bin()
        .args([
            "convert",
            "--read-opt",
            "missing=sidecar",
            "--write-opt",
            "compression=char",
        ])
        .arg(&miss)
        .arg(&dst)
        .output()
        .unwrap();
    assert!(
        conv.status.success(),
        "{}",
        String::from_utf8_lossy(&conv.stderr)
    );
    let codes = bin()
        .args([
            "csv",
            "--read-opt",
            "missing=sidecar",
            "--columns",
            "x__missing",
        ])
        .arg(&dst)
        .output()
        .unwrap();
    let lines: Vec<&str> = std::str::from_utf8(&codes.stdout)
        .unwrap()
        .lines()
        .skip(1)
        .collect();
    assert_eq!(lines.len(), 28);
    assert_eq!((lines[0], lines[27]), ("1", "28"));
    // a failing conversion leaves nothing behind
    let never = dir.join("never.sas7bdat");
    let bad = bin()
        .args(["convert", "--write-opt", "compression=zip"])
        .arg(&miss)
        .arg(&never)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(!never.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
