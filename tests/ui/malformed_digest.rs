use binary_compat::compat_test;

#[compat_test(digest = "abc", samples = 1)]
struct Unsupported;

fn main() {}
