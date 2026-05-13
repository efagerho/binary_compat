#[cfg(feature = "macros")]
#[test]
fn macro_validation_errors_are_clear() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");

    #[cfg(feature = "fixtures")]
    tests.compile_fail("tests/ui-fixtures/*.rs");
}

#[cfg(all(feature = "bincode1", feature = "bincode2"))]
#[test]
fn bincode_auto_requires_explicit_version_when_both_versions_are_enabled() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ui-bincode-ambiguous/Cargo.toml");
    let target_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-bincode-ambiguous");

    let output = std::process::Command::new(env!("CARGO"))
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--offline")
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .expect("failed to run cargo check for bincode ambiguity fixture");

    assert!(
        !output.status.success(),
        "bincode ambiguity fixture unexpectedly passed"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BincodeAutoCompatSerializeRequiresOneBincodeFeatureOrBincodeAttribute"),
        "expected bincode auto-selection diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("Ambiguous"),
        "expected diagnostic to mention the ambiguous type, got:\n{stderr}"
    );
}
