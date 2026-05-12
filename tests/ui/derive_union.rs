use binary_compat::CompatSampler;

#[derive(CompatSampler)]
union Unsupported {
    value: u8,
}

fn main() {}
