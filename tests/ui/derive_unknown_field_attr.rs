use binary_compat::CompatSampler;

#[derive(CompatSampler)]
struct Unsupported {
    #[compat(unknown)]
    value: u8,
}

fn main() {}
