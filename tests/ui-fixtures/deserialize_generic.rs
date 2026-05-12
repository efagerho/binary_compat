#[binary_compat::compat_deserialize_test(
    fixture = "tests/fixtures/manual_fixture.json",
)]
struct Generic<T> {
    value: T,
}

fn main() {}
