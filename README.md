# binary_compat

`binary_compat` helps Rust crates test binary serialization compatibility over
time.

It is built around deterministic samples, stable digests, optional declaration
checks, and checked-in fixtures:

- Binary serialization compatibility tests catch unintended changes to encoded
  bytes.
- Optional declaration checks catch changes to the schema-like Rust structure
  behind those bytes, such as renamed fields or reordered enum variants.
- Deserialization fixture tests prove that current code can still read old wire
  payloads into the same semantic values.

For a practical walkthrough, start with [EXAMPLES.md](EXAMPLES.md). For the
digest and fixture model underneath the API, see
[HOW_IT_WORKS.md](HOW_IT_WORKS.md).

## Quick Start

Most projects keep compatibility tests behind an opt-in feature:

```toml
[dependencies]
binary_compat = { version = "0.1", optional = true, features = ["macros"] }

[features]
compat-tests = ["dep:binary_compat"]
```

Add a generated compatibility test and derive the helper traits for the type
whose wire contract you want to protect. This example protects both the encoded
bytes and the serialized declaration. The declaration check is called
`CompatShape` in the API:

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
))]
pub struct Message {
    pub id: u32,
    pub payload: Vec<u8>,
}

#[cfg(feature = "compat-tests")]
impl binary_compat::CompatSerializer for Message {
    fn compat_serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }
}
```

Run the compatibility tests explicitly:

```sh
cargo test --features compat-tests
```

The placeholder digests fail the first time. Inspect the actual byte digest and
serialized-declaration digest, confirm the current format is the baseline you
want, then replace the placeholders and commit them with the protected type.

By default, `compat_test` samples 1024 values. Override that by adding
`samples = ...` inside the `compat_test` attribute:

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
    samples = 4096,
))]
```

You can also print digests from an ignored helper test:

```rust
#[test]
#[ignore = "prints the current compatibility digests"]
fn bless_message_digests() {
    eprintln!(
        "digest = {}",
        binary_compat::compat_digest_hex::<Message>(binary_compat::DEFAULT_SAMPLES)
    );
    eprintln!(
        "shape_digest = {}",
        binary_compat::compat_shape_digest_hex::<Message>()
    );
}
```

See [EXAMPLES.md#1-add-binary_compat-to-an-existing-project](EXAMPLES.md#1-add-binary_compat-to-an-existing-project)
for the fuller first-use flow, including where serializer derives fit.

## Crate Features

- `macros`: derive `CompatSampler`, `CompatFingerprint`, `CompatShape`, and use
  `compat_test`.
- `compat-tests`: dependency feature alias for `macros`; many crates also use
  this as their project-local opt-in feature name.
- `bincode`: alias for `bincode2`.
- `bincode2`: derive `BincodeSerializer` and `BincodeDeserializer` for
  bincode 2 `Encode`/`Decode` types.
- `bincode1`: derive `BincodeSerializer` and `BincodeDeserializer` for bincode
  1 serde types.
- `wincode`: derive `WincodeSerializer` and `WincodeDeserializer`.
- `fixtures`: generate and validate JSON deserialization compatibility
  fixtures; this also enables `macros`.

If both `bincode1` and `bincode2` are enabled in the same build, add
`#[compat(bincode = "1")]` or `#[compat(bincode = "2")]` to each
`BincodeSerializer` / `BincodeDeserializer` derive.

See [EXAMPLES.md#4-use-bincode-or-wincode-serializer-derives](EXAMPLES.md#4-use-bincode-or-wincode-serializer-derives)
for bincode and wincode examples.

## Core Concepts

- `CompatSampler`: deterministically creates representative values.
- `CompatSerializer`: returns the bytes whose compatibility should be protected.
- `CompatShape`: describes the schema-like Rust declaration behind the binary
  format: struct or enum kind, field names, field order, variant order, and
  field type shapes.
- `CompatDeserializer`: reads compatibility fixture payloads.
- `CompatFingerprint`: computes a stable semantic fingerprint of an in-memory
  value for deserialization fixture validation.

Derived fingerprints support `#[compat(fingerprint_since = N)]` for fields added
after earlier fixtures were generated. Older fixtures keep using their stored
fingerprint version, so the new field is ignored for those fixtures and included
in newer ones.

## Deserialization Fixtures

Serialization tests answer whether current code still writes compatible bytes.
Fixture tests answer whether current code can still read old checked-in bytes.

Use fixtures when changing deserializers, adding fallback readers, or migrating
from one binary format to another. The usual flow is:

- Generate a fixture while the old writer is still available.
- Commit the JSON fixture.
- Keep validating that fixture with the current `CompatDeserializer`.

Attach more than one named fixture when you need to keep validating several wire
generations:

```rust
#[binary_compat::compat_deserialize_test(
    fixtures(
        bincode_v1 = "tests/compat/foo-bincode-v1.json",
        wincode_v2 = "tests/compat/foo-wincode-v2.json",
    )
)]
#[derive(binary_compat::CompatFingerprint)]
struct Foo;
```

The annotated type must implement `CompatDeserializer` and `CompatFingerprint`
in the test build.

The detailed fixture workflow lives in
[EXAMPLES.md#2-add-deserialization-fixtures-to-an-existing-project](EXAMPLES.md#2-add-deserialization-fixtures-to-an-existing-project).
Format migrations are covered in
[EXAMPLES.md#11-change-the-binary-format-and-decode-both-old-and-new-bytes](EXAMPLES.md#11-change-the-binary-format-and-decode-both-old-and-new-bytes),
and field additions with old fixtures are covered in
[EXAMPLES.md#12-add-a-field-while-keeping-old-bytes-readable](EXAMPLES.md#12-add-a-field-while-keeping-old-bytes-readable).

## How It Fits Together

Binary serialization compatibility, declaration compatibility, and binary
deserialization compatibility answer different questions:

- Byte digest: do sampled values still serialize to the same protected bytes?
- Shape digest: did the schema-like Rust declaration behind those bytes stay
  stable?
- Fixture semantic digest: do old bytes still decode to the same in-memory
  meaning?

[HOW_IT_WORKS.md](HOW_IT_WORKS.md) explains how those digests are computed, why
shape is checked separately from bytes, and why deserialization fixtures compare
semantic fingerprints instead of serialized bytes.
