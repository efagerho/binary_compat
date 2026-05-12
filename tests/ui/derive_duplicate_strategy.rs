use binary_compat::CompatSampler;

fn sample_u8<R>(_rng: &mut R) -> u8
where
    R: binary_compat::RngCore + ?Sized,
{
    1
}

#[derive(CompatSampler)]
struct Unsupported {
    #[compat(sample_with = sample_u8, default)]
    value: u8,
}

fn main() {}
