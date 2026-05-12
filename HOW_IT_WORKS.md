# How binary_compat Works

`binary_compat` turns binary compatibility into deterministic tests. The crate
does not try to understand every serialization format directly. Instead, users
define the compatibility surface with traits, and the crate handles sampling,
digesting, fixture generation, and generated test wiring.

## Serialization Compatibility

The serialization path uses three pieces:

- `CompatSampler` creates deterministic sample values.
- `CompatSerializer` converts each sample into the bytes that must stay stable.
- `compat_test` generates a normal Rust test that compares the computed digest
  against a checked-in golden digest.

The default sampler seed is stable:

```text
binary_compat default seed v1
```

The RNG is `ChaCha20Rng`, so the sample stream is intended to be reproducible
across machines and test runs.

When a generated test omits `samples`, it uses `binary_compat::DEFAULT_SAMPLES`,
which is 1024. That keeps normal test loops quick while still covering many
generated values. Raise the sample count for especially large compatibility
surfaces or when you want a denser long-running check.

For `samples = N`, the byte digest is computed as:

```text
h = SHA256(payload_1)
h = SHA256(h || payload_2)
h = SHA256(h || payload_3)
...
h = SHA256(h || payload_N)
```

This chained digest avoids storing every generated payload in the test while
still making every sampled value contribute to the final result.

## Shape-Aware Serialization Compatibility

Byte samples can miss declaration-level changes. For example, these two structs
can serialize to the same bytes in many binary formats:

```rust
struct Old {
    id: u32,
}

struct New {
    user_id: u32,
}
```

The serialized bytes may be identical, but the declared serialized shape changed.

`CompatShape` captures a value-independent public declaration shape: type kind,
type name, field names, field order, field type shapes, enum variant names,
variant order, and variant field shapes. This is intentionally stricter than
pure wire-byte compatibility for formats that do not encode field names.

When `compat_test(shape_digest = "...")` is used, the generated test module
contains two independent tests:

```text
serialization_digest = digest of sampled serialized bytes
shape_digest         = digest of the public declaration shape
```

This keeps the failure mode explicit: a serialization digest mismatch means the
sampled bytes changed, while a shape digest mismatch means the public declaration
shape changed.

## Deserialization Compatibility Fixtures

Serialization stability does not prove that new code can read old bytes. For
that, `binary_compat` uses explicit fixtures.

Fixture generation happens before a migration:

1. Sample a value with `CompatSampler`.
2. Serialize it with the old `CompatSerializer`.
3. Store the payload as hex in JSON.
4. Compute a semantic fingerprint with `CompatFingerprint`.
5. Store the type's `COMPAT_FINGERPRINT_VERSION` in the fixture.
6. Chain payload and semantic digests into the fixture metadata.

Fixture validation happens after a migration:

1. Load the checked-in JSON fixture.
2. Verify `payload_digest` to detect fixture corruption.
3. Decode every old payload with the current `CompatDeserializer`.
4. Fingerprint each decoded value with `CompatFingerprint`, using the fixture's
   stored fingerprint version.
5. Compare the computed semantic digest to the fixture's `semantic_digest`.

`payload_digest` answers: "Was the fixture file altered or corrupted?"

`semantic_digest` answers: "Do the old bytes still decode to the same semantic
values?"

The fingerprint version lets old fixtures ignore fields that were added later.
With `#[compat(fingerprint_since = 2)]`, a field is skipped for version 1
fixtures and included for fixtures generated at version 2 or newer.

## Why Fingerprints Are Separate From Bytes

During migrations, bytes may intentionally change. For example, new writes may
move from bincode to wincode, or a custom fallback decoder may be introduced.

The important deserialization question is not whether current bytes match old
bytes. The important question is whether old bytes decode into the same
in-memory meaning. `CompatFingerprint` is the stable semantic representation
used for that comparison.

## Foreign Types And Overrides

Rust's orphan rules often prevent users from implementing compatibility traits
for foreign types. The derives therefore support field-level escape hatches:

- `#[compat(sample_with = path)]`
- `#[compat(default)]`
- `#[compat(value = expr)]`
- `#[compat(fingerprint_with = path)]`
- `#[compat(fingerprint_skip)]`
- `#[compat(fingerprint_since = N)]`
- `#[compat(shape_with = path)]`
- `#[compat(shape_skip)]`

Sampler-specific keys are ignored by shape and fingerprint derives.
Shape-specific keys are ignored by sampler and fingerprint derives.
Fingerprint-specific keys are ignored by sampler and shape derives.

That means one field can carry all compatibility instructions it needs without
forcing separate wrapper types.

## Supported Built-ins

The crate provides built-ins for common primitives and containers. `CompatShape`
also covers standard-library shapes similar to those supported by
`solana-frozen-abi`, including references, slices, tuples up to arity 12,
`Arc`, `Rc`, `Weak`, `Mutex`, `RwLock`, atomics, `HashMap`, `HashSet`,
`VecDeque`, `Duration`, `SystemTime`, `PathBuf`, `SocketAddr`, and `IpAddr`.

For third-party types, derive the trait if the type is local, or use a field
override if the type is foreign.

## Bincode Version Selection

The crate supports both bincode 1 and bincode 2 behind separate feature gates.

- `bincode1` uses bincode 1's serde API, so derived `BincodeSerializer` requires
  `serde::Serialize` and derived `BincodeDeserializer` requires
  `serde::de::DeserializeOwned`. Deserialization uses bincode 1's legacy fixed
  integer encoding and rejects trailing bytes.
- `bincode2` uses bincode 2's native `Encode` and `Decode` traits.
- `bincode` is kept as an alias for `bincode2`.

The derive expands through private runtime helper traits. If only `bincode1` is
enabled, an unqualified `BincodeSerializer` targets bincode 1. If only
`bincode2` is enabled, it targets bincode 2. In builds that enable both versions,
the unqualified derive is rejected; use `#[compat(bincode = "1")]` or
`#[compat(bincode = "2")]` to choose explicitly for a type.
