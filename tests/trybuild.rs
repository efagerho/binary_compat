#[cfg(feature = "macros")]
#[test]
fn macro_validation_errors_are_clear() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");

    #[cfg(feature = "fixtures")]
    tests.compile_fail("tests/ui-fixtures/*.rs");

    #[cfg(all(feature = "bincode1", feature = "bincode2"))]
    tests.compile_fail("tests/ui-bincode/*.rs");
}
