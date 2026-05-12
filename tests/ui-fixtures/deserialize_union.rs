#[binary_compat::compat_deserialize_test(
    fixture = "tests/fixtures/manual_fixture.json",
)]
union NotSupported {
    value: u32,
}

fn main() {}
