//! End-to-end security regression using a synthetic table and the real CLI binary.
#![cfg(feature = "sas")]
use arrow_array::{Float64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use std::process::Command;
use std::sync::Arc;
use uniformat_sas_arrow::{ArrowWriteOptions, ArrowWriter};

fn fixture(path: &std::path::Path) {
    let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0]))],
    )
    .unwrap();
    let mut writer = ArrowWriter::create(path, schema, ArrowWriteOptions::default()).unwrap();
    writer.write(&batch).unwrap();
    writer.commit().unwrap();
}

#[test]
fn synthetic_conversion_and_resource_errors() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let src = dir.path().join("source.sas7bdat");
    let dst = dir.path().join("private.sas7bdat");
    fixture(&src);
    std::fs::write(&dst, b"previous private contents").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_uniformat"))
        .arg("convert")
        .arg(&src)
        .arg(&dst)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dst).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_uniformat"))
        .arg("csv")
        .arg(&dst)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().lines().count(), 4);
    let before = std::fs::read(&dst).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_uniformat"))
        .args(["convert", "--read-opt", "max_batch_bytes=1"])
        .arg(&src)
        .arg(&dst)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&dst).unwrap(), before);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}
