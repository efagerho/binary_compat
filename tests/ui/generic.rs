use binary_compat::compat_test;

#[compat_test(
    digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    samples = 1,
)]
struct Unsupported<T> {
    value: T,
}

fn main() {}
