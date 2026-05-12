#[binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = 123,
)]
struct Unsupported {
    value: u8,
}

fn main() {}
