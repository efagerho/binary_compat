#[derive(
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
    binary_compat::BincodeSerializer,
)]
struct Ambiguous {
    value: u8,
}

fn main() {}
