use binary_compat::compat_test;

#[compat_test(
    digest = "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef",
    samples = 1,
)]
struct Unsupported;

fn main() {}
