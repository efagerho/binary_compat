#[binary_compat::compat_deserialize_test(
    fixtures(
        legacy = "tests/fixtures/manual_fixture.json",
        legacy = "tests/fixtures/manual_fixture.json",
    )
)]
struct DuplicateFixtureName;

fn main() {}
