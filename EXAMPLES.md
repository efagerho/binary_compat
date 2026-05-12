# binary_compat Examples

This guide shows common ways to use `binary_compat` and how to verify that
serialization and deserialization changes retain compatibility.

For a higher-level explanation of the traits and digest model, see
[HOW_IT_WORKS.md](HOW_IT_WORKS.md). In short, `CompatSampler` creates
deterministic values, `CompatSerializer` writes the protected bytes,
`CompatShape` describes the public declaration shape, `CompatDeserializer` reads
old fixture payloads, and `CompatFingerprint` compares decoded in-memory
meaning.

## 1. Add binary_compat to an Existing Project

Use this when introducing `binary_compat` to a crate that already has normal
builds and tests. In most projects, compatibility tests should be opt-in, so add
`binary_compat` as an optional dependency and expose a crate feature that enables
the generated tests:

```toml
[dependencies]
binary_compat = { version = "0.1", optional = true, features = ["macros"] }

[features]
compat-tests = ["dep:binary_compat"]
```

Then mark the types you want to protect with `cfg_attr`, so normal builds do not
need to compile the compatibility machinery:

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
    pub id: u64,
}
```

This wires the type into compatibility test generation. The type also needs a
`CompatSerializer` implementation; section 3 shows the custom-serializer
pattern, and section 4 shows the built-in bincode and wincode derives.

`digest` protects the bytes produced by `CompatSerializer`; `shape_digest`
protects the public serialized shape of the type, such as field order, field
names, enum variant order, and field types. Run the compatibility tests
explicitly:

```sh
cargo test --features compat-tests
```

The placeholder digests fail on the first run. After checking that the current
bytes and shape are the baseline you want to protect, replace the placeholders
with the actual digests printed by the failures. Commit those digests with the
code they protect.

Use additional features when you want built-in serializer derives:

```toml
binary_compat = { version = "0.1", optional = true, features = ["macros", "bincode2"] }
```

For deserialization fixtures, continue with the next section.

## 2. Add Deserialization Fixtures to an Existing Project

Use this when your crate must prove that current code can still read bytes
written by an older version. Fixtures are checked-in JSON files, so enable them
only in the feature that runs compatibility tests:

```toml
[features]
compat-tests = ["dep:binary_compat", "binary_compat/fixtures"]
```

Attach one or more fixtures to the type that must keep reading old bytes. This
assumes the type also implements or derives `CompatDeserializer` in the test
build:

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_deserialize_test(
    fixtures(
        bincode_v1 = "tests/compat/message-bincode-v1.json",
    )
))]
#[cfg_attr(feature = "compat-tests", derive(binary_compat::CompatFingerprint))]
pub struct Message {
    pub id: u64,
}
```

The fixture test needs `CompatDeserializer` to read each stored payload and
`CompatFingerprint` to check that the decoded in-memory value still has the same
meaning.

Generate fixture files from an ignored test while the old serializer is still
available:

```rust
#[test]
#[ignore = "regenerates the legacy message fixture"]
fn bless_message_fixture() {
    binary_compat::write_default_deserialize_fixture::<Message>(
        "tests/compat/message-bincode-v1.json",
        binary_compat::deserialize_fixture_metadata!("bincode standard"),
    )
    .unwrap();
}
```

Run only the ignored generation test, then commit the JSON fixture:

```sh
cargo test --features compat-tests bless_message_fixture -- --ignored
```

Fixture generation requires `CompatSampler`, `CompatSerializer`, and
`CompatFingerprint`. Fixture validation requires `CompatDeserializer` and
`CompatFingerprint`.

Fixtures also carry a fingerprint version. The version records which fields are
part of the semantic comparison for that fixture generation. A type starts at
version 1; when you later add a field that should not affect old fixtures, mark
that field with `#[compat(fingerprint_since = 2)]`. Version 1 fixtures ignore
the field, while fixtures generated after the change include it.

You do not pass this version to `write_default_deserialize_fixture`; fixture
generation stores the latest `#[compat(fingerprint_since = N)]` value from the
type automatically. Section 12 shows the field-addition workflow in detail.

## 3. Protect a Custom Serializer

Use this when your type already has its own binary writer and you want
`binary_compat` to treat that writer's output as the compatibility contract.

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(binary_compat::CompatSampler))]
pub struct Record {
    pub id: u64,
    pub flags: Vec<bool>,
}

impl Record {
    // The function whose stability I want to protect
    fn to_binary(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&(self.flags.len() as u32).to_le_bytes());
        out.extend(self.flags.iter().map(|flag| u8::from(*flag)));
        out
    }
}

// Implement the CompatSerializer trait using the custom binary serializer
#[cfg(feature = "compat-tests")]
impl binary_compat::CompatSerializer for Record {
    fn compat_serialize(&self) -> Vec<u8> {
        self.to_binary()
    }
}
```

Run:

```sh
cargo test --features compat-tests
```

In this example, `derive(CompatSampler)` generates deterministic `Record`
values from the crate's fixed RNG seed. For each sampled value, the generated
test calls `compat_serialize()`, chains the resulting bytes into a digest, and
compares that digest to the value in the `compat_test` attribute.

The first run fails because the digest is a placeholder. The failure message
prints the actual digest for the current implementation. If the current bytes
are the baseline you want to protect, replace the placeholder digest with that
actual value and commit it. Future test failures mean the sampled serialized
bytes changed.

You can also print the digest from an ignored helper test:

```rust
#[test]
#[ignore = "prints the current compatibility digest"]
fn bless_record_digest() {
    eprintln!(
        "{}",
        binary_compat::compat_digest_hex::<Record>(binary_compat::DEFAULT_SAMPLES)
    );
}
```

## 4. Use Bincode or Wincode Serializer Derives

Use this when your project already serializes with bincode or wincode and you
want `binary_compat` to delegate `CompatSerializer` to that existing library.
Enable the matching feature:

```toml
binary_compat = { version = "0.1", optional = true, features = ["bincode2"] }
```

Then derive:

```rust
#[binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
)]
#[derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
    binary_compat::BincodeSerializer,
    bincode::Encode,
)]
pub struct Message {
    pub id: u32,
    pub ok: bool,
}
```

`BincodeSerializer` implements `CompatSerializer` for the type by delegating to
bincode. With the `bincode2` feature, the type must implement `bincode::Encode`.

For wincode, enable `features = ["wincode"]` and derive
`binary_compat::WincodeSerializer` alongside `wincode::SchemaWrite`:

```rust
#[binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
)]
#[derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
    binary_compat::WincodeSerializer,
    wincode::SchemaWrite,
)]
pub struct Message {
    pub id: u32,
    pub ok: bool,
}
```

`WincodeSerializer` implements `CompatSerializer` for the type by delegating to
wincode, so the type must implement `wincode::SchemaWrite`.

For serde-based bincode 1 types, enable `bincode1`:

```toml
binary_compat = { version = "0.1", optional = true, features = ["bincode1"] }
```

Then derive `BincodeSerializer` alongside serde's `Serialize`:

```rust
#[derive(
    binary_compat::BincodeSerializer,
    serde::Serialize,
)]
pub struct LegacyMessage {
    pub id: u32,
    pub ok: bool,
}
```

If both `bincode1` and `bincode2` are enabled in the same build, the unqualified
derive is ambiguous. Choose explicitly with a container attribute:

```rust
#[derive(
    binary_compat::BincodeSerializer,
    binary_compat::BincodeDeserializer,
    serde::Serialize,
    serde::Deserialize,
)]
#[compat(bincode = "1")]
pub struct LegacyMessage {
    pub id: u32,
    pub ok: bool,
}
```

Use `#[compat(bincode = "2")]` for bincode 2 in builds that enable both
features.

## 5. Protect Bytes and Shape Together

Use this when byte-for-byte output is not enough to describe compatibility. Add
`shape_digest` to detect public declaration shape changes, such as field renames,
field order changes, or enum variant changes. This is stricter than byte
compatibility for formats that do not encode field names.

For example, some binary formats encode struct fields by position. Reordering
two fields with the same wire type can leave sampled bytes unchanged while
changing how those bytes should be interpreted:

```rust
// Old
struct Foo {
    a: u64,
    b: u64,
}

// New
struct Foo {
    b: u64,
    a: u64,
}
```

In that case, the serialization digest may still pass, but the shape digest
fails because the public declaration order changed.

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
))]
pub struct Record {
    pub id: u64,
    pub flags: Vec<bool>,
}
```

With `shape_digest`, the generated test module contains separate tests for the
sampled bytes and the public declaration shape. The first protects writer
output; the second protects the schema-like structure you expect readers to
understand.

To print the shape digest directly:

```rust
#[test]
#[ignore = "prints the current shape digest"]
fn bless_record_shape_digest() {
    eprintln!("{}", binary_compat::compat_shape_digest_hex::<Record>());
}
```

## 6. Handle Foreign Fields

Use this when a protected type contains a field from another crate that does not
implement the `binary_compat` traits. Field overrides let you state how that
foreign value should be sampled, shaped, or fingerprinted.

The derive macro can add bounds such as
`other_crate::Timestamp: CompatSampler`, but it cannot create that
implementation for you. Rust's orphan rules only allow a crate to implement a
trait when the trait or the type is local to that crate. Since `CompatSampler`,
`CompatShape`, and `CompatFingerprint` come from `binary_compat`, and
`other_crate::Timestamp` comes from `other_crate`, your crate cannot write those
impls directly either.

The macro also cannot safely inspect an arbitrary foreign type and infer a
compatibility contract. Private fields may be invisible, constructors may enforce
invariants, and the semantic fingerprint you want may not match the type's
internal layout. Field overrides make the contract explicit at the place where
the foreign value participates in your serialized type.

```rust
#[derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
    binary_compat::CompatFingerprint,
)]
pub struct Event {
    pub id: u64,

    #[compat(
        sample_with = sample_timestamp,
        shape_with = timestamp_shape,
        fingerprint_with = timestamp_fingerprint,
    )]
    pub timestamp: other_crate::Timestamp,

    #[compat(default, shape_skip, fingerprint_skip)]
    pub cache: other_crate::Cache,
}

fn sample_timestamp<R>(rng: &mut R) -> other_crate::Timestamp
where
    R: binary_compat::RngCore + ?Sized,
{
    other_crate::Timestamp::from_seconds(rng.next_u64())
}

fn timestamp_shape() -> Vec<u8> {
    b"other_crate::Timestamp(seconds)".to_vec()
}

fn timestamp_fingerprint(value: &other_crate::Timestamp) -> Vec<u8> {
    value.seconds().to_le_bytes().to_vec()
}
```

Here, `timestamp` gets custom sampling, shape, and fingerprint functions.
`cache` is generated with `Default` and omitted from shape and fingerprint
checks, which is useful for fields that are not part of the compatibility
contract.

## 7. Rename a Field

Use this when you want to rename a Rust field without necessarily changing the
public serialized shape. A rename can be byte-compatible but shape-incompatible;
many binary formats serialize this field by position, not by name:

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
))]
pub struct UserRecord {
    pub user_id: u64,
}
```

After a Rust-only rename:

```rust
#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
))]
pub struct UserRecord {
    pub account_id: u64,
}
```

If the serializer still writes the same bytes, `serialization_digest` passes.
`shape_digest` fails because the public declaration shape changed from `user_id`
to `account_id`.

If the rename is an intentional public schema change, print the new shape digest
and update only `shape_digest`:

```rust
#[test]
#[ignore = "prints the current shape digest"]
fn bless_user_record_shape_digest() {
    eprintln!("{}", binary_compat::compat_shape_digest_hex::<UserRecord>());
}
```

If the rename is internal and the public shape should remain `user_id`, implement
`CompatShape` manually and keep the old field name in the shape:

```rust
#[cfg_attr(feature = "compat-tests", derive(binary_compat::CompatSampler))]
pub struct UserRecord {
    pub account_id: u64,
}

#[cfg(feature = "compat-tests")]
impl binary_compat::CompatShape for UserRecord {
    fn compat_shape() -> Vec<u8> {
        let mut out = Vec::new();

        binary_compat::append_shape_part(&mut out, b"struct");
        binary_compat::append_shape_part(&mut out, b"UserRecord");
        binary_compat::append_shape_part(&mut out, &1_u64.to_le_bytes());

        binary_compat::append_shape_part(&mut out, b"field");
        binary_compat::append_shape_part(&mut out, b"user_id");
        let field_shape = <u64 as binary_compat::CompatShape>::compat_shape();
        binary_compat::append_shape_part(&mut out, &field_shape);

        out
    }
}
```

In that case, keep both the old byte digest and the old shape digest.

## 8. Test a Concrete Generic Instantiation

Use this when the generic type itself is reusable, but compatibility is only
meaningful for concrete instantiations such as `Envelope<Order>`. The derives
support generic types, but `compat_test` is intentionally attached to one
concrete struct or enum, so wrap the instantiation in a small test-only newtype.

```rust
#[derive(binary_compat::CompatSampler, binary_compat::CompatShape)]
pub struct Envelope<T> {
    pub id: u64,
    pub payload: T,
}

#[derive(binary_compat::CompatSampler, binary_compat::CompatShape)]
pub struct Order {
    pub quantity: u32,
}

#[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
    digest = "0000000000000000000000000000000000000000000000000000000000000000",
    shape_digest = "0000000000000000000000000000000000000000000000000000000000000000",
))]
#[cfg_attr(feature = "compat-tests", derive(
    binary_compat::CompatSampler,
    binary_compat::CompatShape,
))]
struct OrderEnvelope(Envelope<Order>);
```

Then implement or derive `CompatSerializer` for the wrapper in the same way you
would for any other protected type. Add another wrapper for each generic
instantiation that has its own compatibility contract.

## 9. Change the Serializer and Keep Written Bytes Compatible

Use this when you change writer code or serializer configuration and the new
serializer is supposed to keep producing compatible bytes and shape for sampled
values.

1. Change the serialization implementation.

2. Run the compatibility tests:

   ```sh
   cargo test --features compat-tests
   ```

3. If the digest and shape tests still pass, sampled write compatibility is
   retained.

4. If a digest fails, inspect the diff in writer behavior.

For formats that must remain byte-for-byte compatible forever, do not update the
digest. Fix the serializer instead.

## 10. Change the Deserializer and Keep Old Bytes Readable

Use this when you change reader code or deserializer configuration and the new
deserializer is supposed to keep reading old fixture bytes into the same semantic
in-memory values.

1. Change the deserialization implementation.

2. Run the fixture tests:

   ```sh
   cargo test --features fixtures
   ```

3. If the fixtures still pass, deserialization compatibility is retained.

4. If a fixture fails with a decode error, the reader no longer accepts old
   bytes. Restore old-format read support or add fallback decoding.

5. If a fixture fails with a semantic digest mismatch, old bytes still parse but
   now mean something different. Fix the deserializer or the fingerprint contract.

Do not regenerate old fixtures to make this test pass. The fixture is the old
wire contract; if it fails unexpectedly, fix the deserializer instead.

## 11. Change the Binary Format and Decode Both Old and New Bytes

Use this when new writes move to a different format, but readers must continue
to accept old payloads. Implement one combined read function that tries the new
decoder first and falls back to the legacy decoder, then validate fixtures from
both generations:

```rust
#[binary_compat::compat_deserialize_test(
    fixtures(
        bincode_v1 = "tests/compat/foo-bincode-v1.json",
        wincode_v2 = "tests/compat/foo-wincode-v2.json",
    )
)]
#[derive(binary_compat::CompatFingerprint)]
pub struct Foo {
    pub id: u32,
    pub tags: Vec<String>,
}

impl Foo {
    pub fn decode_current_or_legacy(bytes: &[u8]) -> Result<Self, DecodeError> {
        decode_wincode(bytes).or_else(|_| decode_legacy_bincode(bytes))
    }
}

impl binary_compat::CompatDeserializer for Foo {
    type Error = DecodeError;

    fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::decode_current_or_legacy(bytes)
    }
}
```

This keeps the migration behavior in the same function your application can use,
while `CompatDeserializer` stays as the test-facing adapter. The old fixture
proves legacy payloads still work; the new fixture proves the current format also
decodes to the intended in-memory values.

To add the new fixture, use the ignored-test workflow from section 2 again: keep
the existing fixture file unchanged and generate a second file after switching
`CompatSerializer` to the new format:

```rust
#[test]
#[ignore = "regenerates the current wincode fixture"]
fn bless_foo_wincode_fixture() {
    binary_compat::write_default_deserialize_fixture::<Foo>(
        "tests/compat/foo-wincode-v2.json",
        binary_compat::deserialize_fixture_metadata!("wincode"),
    )
    .unwrap();
}
```

Run only the ignored generation test:

```sh
cargo test --features fixtures bless_foo_wincode_fixture -- --ignored
```

Then commit both fixture files and list both in `fixtures(...)`. Do not overwrite
`foo-bincode-v1.json`; keeping that file is what proves the legacy format remains
readable.

## 12. Add a Field While Keeping Old Bytes Readable

Use this when a new field should get a default value while old payloads remain
readable. Generate a fixture before the change, keep validating it after the
change, and make the fingerprint contract explicit for the new field.

Before adding the field:

```rust
#[derive(
    binary_compat::CompatSampler,
    binary_compat::CompatFingerprint,
    binary_compat::BincodeSerializer,
    bincode::Encode,
)]
pub struct Account {
    pub id: u64,
}

#[test]
#[ignore = "regenerates the legacy account fixture"]
fn bless_account_fixture() {
    binary_compat::write_default_deserialize_fixture::<Account>(
        "tests/compat/account-v1.json",
        binary_compat::deserialize_fixture_metadata!("bincode standard"),
    )
    .unwrap();
}
```

After adding the field, make the current deserializer supply the value that old
payloads should mean. Mark the new field with the fingerprint version where it
became part of the semantic contract:

```rust
#[derive(binary_compat::CompatFingerprint)]
pub struct Account {
    pub id: u64,

    #[compat(fingerprint_since = 2)]
    pub label: Option<String>,
}

impl binary_compat::CompatDeserializer for Account {
    type Error = DecodeError;

    fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
        let legacy = decode_account_v1(bytes)?;
        Ok(Self {
            id: legacy.id,
            label: None,
        })
    }
}
```

The legacy fixture was generated before `label` existed, so it has fingerprint
version 1 and ignores `label` when checked. You do not set the version manually
when generating the next fixture: because `Account` now contains
`#[compat(fingerprint_since = 2)]`, the generated fixture stores fingerprint
version 2 automatically and includes `label` in the semantic digest.

Keep the fixture test attached to `Account`:

```rust
#[binary_compat::compat_deserialize_test(
    fixtures(
        legacy_v1 = "tests/compat/account-v1.json",
    )
)]
#[derive(binary_compat::CompatFingerprint)]
pub struct Account {
    pub id: u64,
    #[compat(fingerprint_since = 2)]
    pub label: Option<String>,
}
```

If the fixture passes, old bytes still decode to the intended in-memory meaning.
When you later generate fixtures for the new format, keep the old fixture and
add the new one with another name:

```rust
#[binary_compat::compat_deserialize_test(
    fixtures(
        legacy_v1 = "tests/compat/account-v1.json",
        after_label_v2 = "tests/compat/account-v2-label.json",
    )
)]
#[derive(binary_compat::CompatFingerprint)]
pub struct Account {
    pub id: u64,
    #[compat(fingerprint_since = 2)]
    pub label: Option<String>,
}
```

Each named fixture becomes its own generated test, so failures identify which
compatibility generation broke.

## 13. Triage Compatibility Failures

Use this when a compatibility test fails and you need to decide whether to fix
the implementation, update a digest, or add migration support:

| Failure | Meaning | Usual next step |
| --- | --- | --- |
| `serialization_digest` failed, `shape_digest` passed | Sampled writer bytes changed without a public declaration shape change. | Inspect serializer/config changes. Keep the old digest if byte compatibility is required. |
| `shape_digest` failed, `serialization_digest` passed | Public declaration shape changed, but sampled bytes did not. | Decide whether this is a public schema change. If yes, update only `shape_digest`; if not, preserve the old shape manually. |
| Both serialization and shape tests failed | The writer bytes and public declaration shape both changed. | Treat this as a wire-format migration; generate fixtures before removing old read support. |
| `compat_deserialize_test` failed with decode error | The current reader cannot parse an old fixture payload. | Add fallback decoding or restore support for the old format. |
| `compat_deserialize_test` failed with semantic digest mismatch | Old bytes parse, but decode to different in-memory meaning. | Fix `CompatDeserializer` or `CompatFingerprint` so the intended semantics are preserved. |

## 14. Compatibility Checklist

Use this before merging any serialization or deserialization change:

- Run the existing serialization digest tests.
- If a digest changed, decide whether the byte or shape change is intentional.
- For intentional write-format changes, generate and commit a fixture before
  deleting the old writer behavior.
- Add or update a `CompatDeserializer` implementation that reads the old bytes.
- Run fixture validation tests.
- Commit fixture files and digest updates together with the migration code.
