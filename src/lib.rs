//! Deterministic digest tests for binary serialization compatibility.
//!
//! `binary_compat` helps crates detect unintended changes in serialized bytes.
//! Implement or derive [`CompatSampler`], implement [`CompatSerializer`] for a
//! concrete type, then use `#[binary_compat::compat_test(...)]` with the
//! `macros` feature to generate a golden digest test.
//!
//! ```rust,ignore
//! #[cfg_attr(feature = "compat-tests", binary_compat::compat_test(
//!     digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
//! ))]
//! #[cfg_attr(feature = "compat-tests", derive(binary_compat::CompatSampler))]
//! pub struct Message {
//!     id: u32,
//! }
//!
//! #[cfg(feature = "compat-tests")]
//! impl binary_compat::CompatSerializer for Message {
//!     fn compat_serialize(&self) -> Vec<u8> {
//!         self.id.to_le_bytes().to_vec()
//!     }
//! }
//! ```
//!
//! The `bincode` and `wincode` features also expose serializer derives:
//!
//! ```rust,ignore
//! #[derive(binary_compat::BincodeSerializer, bincode::Encode)]
//! struct BincodeMessage {
//!     id: u32,
//! }
//!
//! #[derive(binary_compat::WincodeSerializer, wincode::SchemaWrite)]
//! struct WincodeMessage {
//!     id: u32,
//! }
//! ```
//!
//! With the `fixtures` feature, the same sampler can generate old wire payloads
//! and later verify that a new deserializer still reads them into the same
//! semantic values.

use rand_chacha::ChaCha20Rng;
pub use rand_core::RngCore;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};

#[cfg(feature = "macros")]
pub use binary_compat_macros::{CompatFingerprint, CompatSampler, CompatShape, compat_test};

#[cfg(any(feature = "bincode", feature = "bincode1", feature = "bincode2"))]
pub use binary_compat_macros::{BincodeDeserializer, BincodeSerializer};

#[cfg(feature = "wincode")]
pub use binary_compat_macros::{WincodeDeserializer, WincodeSerializer};

#[cfg(feature = "fixtures")]
pub use binary_compat_macros::compat_deserialize_test;

#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "bincode2")]
    pub use bincode;

    #[cfg(feature = "bincode1")]
    pub use bincode1;

    #[cfg(any(feature = "bincode1", feature = "fixtures"))]
    pub use serde;

    #[cfg(feature = "wincode")]
    pub use wincode;

    #[cfg(feature = "bincode1")]
    pub trait Bincode1CompatSerialize {
        fn bincode_compat_serialize(&self) -> Vec<u8>;
    }

    #[cfg(feature = "bincode1")]
    impl<T> Bincode1CompatSerialize for T
    where
        T: serde::Serialize,
    {
        fn bincode_compat_serialize(&self) -> Vec<u8> {
            bincode1::serialize(self).expect("binary_compat bincode 1 serialization failed")
        }
    }

    #[cfg(feature = "bincode2")]
    pub trait Bincode2CompatSerialize {
        fn bincode_compat_serialize(&self) -> Vec<u8>;
    }

    #[cfg(feature = "bincode2")]
    impl<T> Bincode2CompatSerialize for T
    where
        for<'a> &'a T: bincode::Encode,
    {
        fn bincode_compat_serialize(&self) -> Vec<u8> {
            bincode::encode_to_vec(self, bincode::config::standard())
                .expect("binary_compat bincode 2 serialization failed")
        }
    }

    #[cfg(all(feature = "bincode1", not(feature = "bincode2")))]
    pub use Bincode1CompatSerialize as BincodeAutoCompatSerializeRequiresOneBincodeFeatureOrBincodeAttribute;
    #[cfg(all(feature = "bincode2", not(feature = "bincode1")))]
    pub use Bincode2CompatSerialize as BincodeAutoCompatSerializeRequiresOneBincodeFeatureOrBincodeAttribute;
    #[cfg(all(feature = "bincode1", feature = "bincode2"))]
    pub trait BincodeAutoCompatSerializeRequiresOneBincodeFeatureOrBincodeAttribute {
        fn bincode_compat_serialize(&self) -> Vec<u8>;
    }

    #[cfg(feature = "bincode1")]
    pub trait Bincode1CompatDeserialize: Sized {
        type Error: core::fmt::Debug;

        fn bincode_compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error>;
    }

    #[cfg(feature = "bincode1")]
    impl<T> Bincode1CompatDeserialize for T
    where
        T: serde::de::DeserializeOwned,
    {
        type Error = bincode1::Error;

        fn bincode_compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
            use bincode1::Options as _;

            bincode1::DefaultOptions::new()
                .with_fixint_encoding()
                .reject_trailing_bytes()
                .deserialize(bytes)
        }
    }

    #[cfg(feature = "bincode2")]
    pub trait Bincode2CompatDeserialize: Sized {
        type Error: core::fmt::Debug;

        fn bincode_compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error>;
    }

    #[cfg(feature = "bincode2")]
    impl<T> Bincode2CompatDeserialize for T
    where
        T: bincode::Decode<()>,
    {
        type Error = bincode::error::DecodeError;

        fn bincode_compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
            let (value, bytes_read) =
                bincode::decode_from_slice::<Self, _>(bytes, bincode::config::standard())?;

            if bytes_read != bytes.len() {
                return Err(bincode::error::DecodeError::OtherString(format!(
                    "binary_compat bincode 2 deserializer left {} trailing bytes",
                    bytes.len() - bytes_read,
                )));
            }

            Ok(value)
        }
    }

    #[cfg(all(feature = "bincode1", not(feature = "bincode2")))]
    pub use Bincode1CompatDeserialize as BincodeAutoCompatDeserializeRequiresOneBincodeFeatureOrBincodeAttribute;
    #[cfg(all(feature = "bincode2", not(feature = "bincode1")))]
    pub use Bincode2CompatDeserialize as BincodeAutoCompatDeserializeRequiresOneBincodeFeatureOrBincodeAttribute;
    #[cfg(all(feature = "bincode1", feature = "bincode2"))]
    pub trait BincodeAutoCompatDeserializeRequiresOneBincodeFeatureOrBincodeAttribute:
        Sized
    {
        type Error: core::fmt::Debug;

        fn bincode_compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error>;
    }
}

/// Default number of samples used by `#[compat_test]` and fixture helpers.
pub const DEFAULT_SAMPLES: usize = 1024;

/// Default deterministic seed used by [`compat_digest`].
pub const DEFAULT_SEED: [u8; 32] = *b"binary_compat default seed v1!!!";

/// Version of the digest chaining algorithm implemented by this crate.
pub const DIGEST_ALGORITHM_VERSION: u32 = 1;

/// Version of the shape digest algorithm implemented by this crate.
pub const SHAPE_DIGEST_ALGORITHM_VERSION: u32 = 1;

/// Deterministically creates representative values for compatibility testing.
///
/// Implementations are provided for primitives, `String`, `Option<T>`,
/// `Result<T, E>`, `Vec<T>`, `Box<T>`, `Box<[T]>`, `Arc<T>`, `Rc<T>`,
/// `[T; N]`, `PhantomData<T>`, `BTreeMap<K, V>`, `BTreeSet<T>`,
/// `VecDeque<T>`, and tuples of arity 1–12.
///
/// [`CompatShape`] covers a broader set of stdlib types — `HashMap`, `HashSet`,
/// `Mutex`, `RwLock`, atomics, `Weak`, `Once`, `PathBuf`, `SystemTime`, etc. —
/// that cannot be deterministically sampled without nondeterministic iteration
/// order, locking, or other side effects. Use `#[compat(sample_with = ...)]`
/// on fields of those types to provide a custom sampler.
///
/// Sampling of `usize` and `isize` is restricted to values that fit in `u32`
/// / `i32` so that sampled values are bit-identical across 32-bit and 64-bit
/// targets. Use `#[compat(sample_with = ...)]` if a wider range is needed
/// on 64-bit hosts.
pub trait CompatSampler: Sized {
    /// Create one sample value from the provided deterministic RNG.
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized;
}

impl CompatSampler for () {
    fn compat_sample<R>(_rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
    }
}

impl CompatSampler for bool {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        rng.next_u32() & 1 == 1
    }
}

macro_rules! impl_unsigned_sampler {
    ($($ty:ty),* $(,)?) => {
        $(
            impl CompatSampler for $ty {
                fn compat_sample<R>(rng: &mut R) -> Self
                where
                    R: RngCore + ?Sized,
                {
                    rng.next_u64() as Self
                }
            }
        )*
    };
}

macro_rules! impl_signed_sampler {
    ($($ty:ty),* $(,)?) => {
        $(
            impl CompatSampler for $ty {
                fn compat_sample<R>(rng: &mut R) -> Self
                where
                    R: RngCore + ?Sized,
                {
                    rng.next_u64() as Self
                }
            }
        )*
    };
}

impl_unsigned_sampler!(u8, u16, u32, u64);
impl_signed_sampler!(i8, i16, i32, i64);

impl CompatSampler for usize {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        rng.next_u32() as Self
    }
}

impl CompatSampler for isize {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        rng.next_u32() as i32 as Self
    }
}

impl CompatSampler for u128 {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        let mut bytes = [0; 16];
        rng.fill_bytes(&mut bytes);
        Self::from_le_bytes(bytes)
    }
}

impl CompatSampler for i128 {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        u128::compat_sample(rng) as Self
    }
}

impl CompatSampler for f32 {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        Self::from_bits(rng.next_u32())
    }
}

impl CompatSampler for f64 {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        Self::from_bits(rng.next_u64())
    }
}

impl CompatSampler for char {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        let byte = b' ' + (rng.next_u32() % 95) as u8;
        byte as char
    }
}

impl CompatSampler for String {
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        let len = sample_len(rng);
        (0..len).map(|_| char::compat_sample(rng)).collect()
    }
}

impl<T> CompatSampler for Option<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        bool::compat_sample(rng).then(|| T::compat_sample(rng))
    }
}

impl<T, E> CompatSampler for Result<T, E>
where
    T: CompatSampler,
    E: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        if bool::compat_sample(rng) {
            Ok(T::compat_sample(rng))
        } else {
            Err(E::compat_sample(rng))
        }
    }
}

impl<T> CompatSampler for Vec<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        (0..sample_len(rng))
            .map(|_| T::compat_sample(rng))
            .collect()
    }
}

impl<T> CompatSampler for Box<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        Box::new(T::compat_sample(rng))
    }
}

impl<T, const N: usize> CompatSampler for [T; N]
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        std::array::from_fn(|_| T::compat_sample(rng))
    }
}

impl<T> CompatSampler for std::marker::PhantomData<T> {
    fn compat_sample<R>(_rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        Self
    }
}

impl<K, V> CompatSampler for std::collections::BTreeMap<K, V>
where
    K: CompatSampler + Ord,
    V: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        (0..sample_len(rng))
            .map(|_| (K::compat_sample(rng), V::compat_sample(rng)))
            .collect()
    }
}

impl<T> CompatSampler for std::collections::BTreeSet<T>
where
    T: CompatSampler + Ord,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        (0..sample_len(rng))
            .map(|_| T::compat_sample(rng))
            .collect()
    }
}

impl<T> CompatSampler for std::collections::VecDeque<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        (0..sample_len(rng))
            .map(|_| T::compat_sample(rng))
            .collect()
    }
}

impl<T> CompatSampler for std::sync::Arc<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        std::sync::Arc::new(T::compat_sample(rng))
    }
}

impl<T> CompatSampler for std::rc::Rc<T>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        std::rc::Rc::new(T::compat_sample(rng))
    }
}

impl<T> CompatSampler for Box<[T]>
where
    T: CompatSampler,
{
    fn compat_sample<R>(rng: &mut R) -> Self
    where
        R: RngCore + ?Sized,
    {
        (0..sample_len(rng))
            .map(|_| T::compat_sample(rng))
            .collect()
    }
}

macro_rules! impl_tuple_sampler {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> CompatSampler for ($($name,)+)
        where
            $($name: CompatSampler),+
        {
            fn compat_sample<R>(rng: &mut R) -> Self
            where
                R: RngCore + ?Sized,
            {
                ($($name::compat_sample(rng),)+)
            }
        }
    };
}

impl_tuple_sampler!(A);
impl_tuple_sampler!(A, B);
impl_tuple_sampler!(A, B, C);
impl_tuple_sampler!(A, B, C, D);
impl_tuple_sampler!(A, B, C, D, E);
impl_tuple_sampler!(A, B, C, D, E, F);
impl_tuple_sampler!(A, B, C, D, E, F, G);
impl_tuple_sampler!(A, B, C, D, E, F, G, H);
impl_tuple_sampler!(A, B, C, D, E, F, G, H, I);
impl_tuple_sampler!(A, B, C, D, E, F, G, H, I, J);
impl_tuple_sampler!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple_sampler!(A, B, C, D, E, F, G, H, I, J, K, L);

fn sample_len<R>(rng: &mut R) -> usize
where
    R: RngCore + ?Sized,
{
    (rng.next_u32() % 16) as usize
}

/// Serializes the bytes whose binary compatibility should be protected.
pub trait CompatSerializer {
    /// Return the exact serialized bytes that should remain stable.
    fn compat_serialize(&self) -> Vec<u8>;
}

/// Describes the stable shape of a type's serialized representation.
///
/// Shape is the declared structure of the compatibility surface: type kind,
/// field names, field order, enum variants, and field type shapes. It is
/// intentionally value-independent, so it catches changes that sampled
/// serialized bytes may miss.
pub trait CompatShape {
    /// Return stable bytes representing the serialized shape of this type.
    fn compat_shape() -> Vec<u8>;
}

/// Deserializes bytes that should remain readable for backwards compatibility.
pub trait CompatDeserializer: Sized {
    /// The deserializer error type.
    type Error: core::fmt::Debug;

    /// Decode one value from the exact fixture payload bytes.
    fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error>;
}

/// Produces a stable semantic fingerprint of a value.
///
/// This is intentionally separate from [`CompatSerializer`]: fixture
/// deserialization tests compare fingerprints so the wire format can change as
/// long as the decoded in-memory meaning is preserved.
///
/// Implementations are provided for the same set of types as [`CompatSampler`]:
/// primitives, `String`, `Option<T>`, `Result<T, E>`, `Vec<T>`, `Box<T>`,
/// `Box<[T]>`, `Arc<T>`, `Rc<T>`, `[T; N]`, `PhantomData<T>`,
/// `BTreeMap<K, V>`, `BTreeSet<T>`, `VecDeque<T>`, and tuples of arity 1–12.
/// Use `#[compat(fingerprint_with = ...)]` for fields whose type is only
/// covered by [`CompatShape`].
pub trait CompatFingerprint {
    /// Latest semantic fingerprint version produced by this implementation.
    const COMPAT_FINGERPRINT_VERSION: u32 = 1;

    /// Return stable bytes representing the value's semantic meaning.
    fn compat_fingerprint(&self) -> Vec<u8>;

    /// Return a semantic fingerprint for a specific fixture generation.
    fn compat_fingerprint_with(&self, _context: FingerprintContext) -> Vec<u8> {
        self.compat_fingerprint()
    }
}

/// Context passed while computing a semantic fingerprint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FingerprintContext {
    version: u32,
}

impl FingerprintContext {
    /// Create a fingerprint context for a fixture generation.
    pub const fn new(version: u32) -> Self {
        Self { version }
    }

    /// Create a fingerprint context for the latest version of `T`.
    pub const fn latest<T>() -> Self
    where
        T: CompatFingerprint + ?Sized,
    {
        Self {
            version: T::COMPAT_FINGERPRINT_VERSION,
        }
    }

    /// The fixture fingerprint version being checked.
    pub const fn version(self) -> u32 {
        self.version
    }
}

const fn max_fingerprint_versions<const N: usize>(values: [u32; N]) -> u32 {
    let mut max = 1;
    let mut index = 0;
    while index < N {
        if values[index] > max {
            max = values[index];
        }
        index += 1;
    }
    max
}

/// Appends a length-prefixed fingerprint part to an aggregate fingerprint.
///
/// Derives use this for struct fields, enum fields, tuple elements, and
/// collection entries so adjacent variable-length parts cannot collide.
pub fn append_fingerprint_part(out: &mut Vec<u8>, part: &[u8]) {
    out.extend_from_slice(&(part.len() as u64).to_le_bytes());
    out.extend_from_slice(part);
}

/// Appends a length-prefixed shape part to an aggregate shape.
pub fn append_shape_part(out: &mut Vec<u8>, part: &[u8]) {
    append_fingerprint_part(out, part);
}

fn append_shape_str(out: &mut Vec<u8>, part: &str) {
    append_shape_part(out, part.as_bytes());
}

fn append_shape_u64(out: &mut Vec<u8>, value: u64) {
    append_shape_part(out, &value.to_le_bytes());
}

fn leaf_shape(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    append_shape_str(&mut out, "leaf");
    append_shape_str(&mut out, name);
    out
}

fn constructor_shape(name: &str, parts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    append_shape_str(&mut out, "constructor");
    append_shape_str(&mut out, name);
    append_shape_u64(&mut out, parts.len() as u64);
    for part in parts {
        append_shape_part(&mut out, part);
    }
    out
}

macro_rules! impl_leaf_shape {
    ($($ty:ty => $name:literal),* $(,)?) => {
        $(
            impl CompatShape for $ty {
                fn compat_shape() -> Vec<u8> {
                    leaf_shape($name)
                }
            }
        )*
    };
}

impl_leaf_shape!(
    () => "()",
    bool => "bool",
    char => "char",
    u8 => "u8",
    u16 => "u16",
    u32 => "u32",
    u64 => "u64",
    u128 => "u128",
    usize => "usize",
    i8 => "i8",
    i16 => "i16",
    i32 => "i32",
    i64 => "i64",
    i128 => "i128",
    isize => "isize",
    f32 => "f32",
    f64 => "f64",
    String => "String",
);

impl<T> CompatShape for Option<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Option", &[T::compat_shape()])
    }
}

impl<T, E> CompatShape for Result<T, E>
where
    T: CompatShape,
    E: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Result", &[T::compat_shape(), E::compat_shape()])
    }
}

impl<T> CompatShape for Vec<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Vec", &[T::compat_shape()])
    }
}

impl<T> CompatShape for Box<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Box", &[T::compat_shape()])
    }
}

impl<T, const N: usize> CompatShape for [T; N]
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        let mut out = Vec::new();
        append_shape_str(&mut out, "array");
        append_shape_u64(&mut out, N as u64);
        append_shape_part(&mut out, &T::compat_shape());
        out
    }
}

impl<T> CompatShape for std::marker::PhantomData<T> {
    fn compat_shape() -> Vec<u8> {
        constructor_shape("PhantomData", &[])
    }
}

impl<K, V> CompatShape for std::collections::BTreeMap<K, V>
where
    K: CompatShape,
    V: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("BTreeMap", &[K::compat_shape(), V::compat_shape()])
    }
}

impl<T> CompatShape for std::collections::BTreeSet<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("BTreeSet", &[T::compat_shape()])
    }
}

macro_rules! impl_tuple_shape {
    ($arity:expr, $($name:ident),+ $(,)?) => {
        impl<$($name),+> CompatShape for ($($name,)+)
        where
            $($name: CompatShape),+
        {
            fn compat_shape() -> Vec<u8> {
                let mut out = Vec::new();
                append_shape_str(&mut out, "tuple");
                append_shape_u64(&mut out, $arity);
                $(
                    append_shape_part(&mut out, &$name::compat_shape());
                )+
                out
            }
        }
    };
}

impl_tuple_shape!(1, A);
impl_tuple_shape!(2, A, B);
impl_tuple_shape!(3, A, B, C);
impl_tuple_shape!(4, A, B, C, D);
impl_tuple_shape!(5, A, B, C, D, E);
impl_tuple_shape!(6, A, B, C, D, E, F);
impl_tuple_shape!(7, A, B, C, D, E, F, G);
impl_tuple_shape!(8, A, B, C, D, E, F, G, H);
impl_tuple_shape!(9, A, B, C, D, E, F, G, H, I);
impl_tuple_shape!(10, A, B, C, D, E, F, G, H, I, J);
impl_tuple_shape!(11, A, B, C, D, E, F, G, H, I, J, K);
impl_tuple_shape!(12, A, B, C, D, E, F, G, H, I, J, K, L);

impl CompatShape for str {
    fn compat_shape() -> Vec<u8> {
        leaf_shape("str")
    }
}

impl<T> CompatShape for [T]
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("slice", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for &T
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("&", &[T::compat_shape()])
    }
}

impl<T> CompatShape for Box<[T]>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Box<[T]>", &[T::compat_shape()])
    }
}

impl<T> CompatShape for Box<dyn Fn(&mut T) + Send + Sync>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Box<dyn Fn(&mut T) + Send + Sync>", &[T::compat_shape()])
    }
}

impl<T, U> CompatShape for Box<dyn Fn(&mut T, U) + Send + Sync>
where
    T: CompatShape,
    U: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape(
            "Box<dyn Fn(&mut T, U) + Send + Sync>",
            &[T::compat_shape(), U::compat_shape()],
        )
    }
}

impl<T: ?Sized> CompatShape for std::sync::Arc<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Arc", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for std::rc::Rc<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Rc", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for std::sync::Weak<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("std::sync::Weak", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for std::rc::Weak<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("std::rc::Weak", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for std::sync::Mutex<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("Mutex", &[T::compat_shape()])
    }
}

impl<T: ?Sized> CompatShape for std::sync::RwLock<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("RwLock", &[T::compat_shape()])
    }
}

impl<T> CompatShape for std::sync::OnceLock<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("OnceLock", &[T::compat_shape()])
    }
}

impl<K, V, S> CompatShape for std::collections::HashMap<K, V, S>
where
    K: CompatShape,
    V: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("HashMap", &[K::compat_shape(), V::compat_shape()])
    }
}

impl<T, S> CompatShape for std::collections::HashSet<T, S>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("HashSet", &[T::compat_shape()])
    }
}

impl<T> CompatShape for std::collections::VecDeque<T>
where
    T: CompatShape,
{
    fn compat_shape() -> Vec<u8> {
        constructor_shape("VecDeque", &[T::compat_shape()])
    }
}

impl_leaf_shape!(
    std::time::Duration => "std::time::Duration",
    std::time::SystemTime => "std::time::SystemTime",
    std::sync::Once => "std::sync::Once",
    std::path::PathBuf => "std::path::PathBuf",
    std::net::SocketAddr => "std::net::SocketAddr",
    std::net::IpAddr => "std::net::IpAddr",
    std::sync::atomic::AtomicU8 => "std::sync::atomic::AtomicU8",
    std::sync::atomic::AtomicU16 => "std::sync::atomic::AtomicU16",
    std::sync::atomic::AtomicU32 => "std::sync::atomic::AtomicU32",
    std::sync::atomic::AtomicU64 => "std::sync::atomic::AtomicU64",
    std::sync::atomic::AtomicUsize => "std::sync::atomic::AtomicUsize",
    std::sync::atomic::AtomicI8 => "std::sync::atomic::AtomicI8",
    std::sync::atomic::AtomicI16 => "std::sync::atomic::AtomicI16",
    std::sync::atomic::AtomicI32 => "std::sync::atomic::AtomicI32",
    std::sync::atomic::AtomicI64 => "std::sync::atomic::AtomicI64",
    std::sync::atomic::AtomicIsize => "std::sync::atomic::AtomicIsize",
    std::sync::atomic::AtomicBool => "std::sync::atomic::AtomicBool",
);

impl CompatFingerprint for () {
    fn compat_fingerprint(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl CompatFingerprint for bool {
    fn compat_fingerprint(&self) -> Vec<u8> {
        vec![u8::from(*self)]
    }
}

macro_rules! impl_fixed_fingerprint {
    ($($ty:ty),* $(,)?) => {
        $(
            impl CompatFingerprint for $ty {
                fn compat_fingerprint(&self) -> Vec<u8> {
                    self.to_le_bytes().to_vec()
                }
            }
        )*
    };
}

impl_fixed_fingerprint!(u8, u16, u32, u64, i8, i16, i32, i64);

impl CompatFingerprint for usize {
    fn compat_fingerprint(&self) -> Vec<u8> {
        (*self as u64).to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for isize {
    fn compat_fingerprint(&self) -> Vec<u8> {
        (*self as i64).to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for u128 {
    fn compat_fingerprint(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for i128 {
    fn compat_fingerprint(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for f32 {
    fn compat_fingerprint(&self) -> Vec<u8> {
        self.to_bits().to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for f64 {
    fn compat_fingerprint(&self) -> Vec<u8> {
        self.to_bits().to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for char {
    fn compat_fingerprint(&self) -> Vec<u8> {
        (*self as u32).to_le_bytes().to_vec()
    }
}

impl CompatFingerprint for String {
    fn compat_fingerprint(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl<T> CompatFingerprint for Option<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Some(value) => {
                out.push(1);
                append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
            }
            None => out.push(0),
        }
        out
    }
}

impl<T, E> CompatFingerprint for Result<T, E>
where
    T: CompatFingerprint,
    E: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 =
        if T::COMPAT_FINGERPRINT_VERSION > E::COMPAT_FINGERPRINT_VERSION {
            T::COMPAT_FINGERPRINT_VERSION
        } else {
            E::COMPAT_FINGERPRINT_VERSION
        };

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Ok(value) => {
                out.push(1);
                append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
            }
            Err(error) => {
                out.push(0);
                append_fingerprint_part(&mut out, &error.compat_fingerprint_with(context));
            }
        }
        out
    }
}

impl<T> CompatFingerprint for Vec<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u64).to_le_bytes());
        for value in self {
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

impl<T> CompatFingerprint for Box<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        (**self).compat_fingerprint_with(context)
    }
}

impl<T, const N: usize> CompatFingerprint for [T; N]
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(N as u64).to_le_bytes());
        for value in self {
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

impl<T> CompatFingerprint for std::marker::PhantomData<T> {
    fn compat_fingerprint(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl<K, V> CompatFingerprint for std::collections::BTreeMap<K, V>
where
    K: CompatFingerprint + Ord,
    V: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 =
        if K::COMPAT_FINGERPRINT_VERSION > V::COMPAT_FINGERPRINT_VERSION {
            K::COMPAT_FINGERPRINT_VERSION
        } else {
            V::COMPAT_FINGERPRINT_VERSION
        };

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u64).to_le_bytes());
        for (key, value) in self {
            append_fingerprint_part(&mut out, &key.compat_fingerprint_with(context));
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

impl<T> CompatFingerprint for std::collections::BTreeSet<T>
where
    T: CompatFingerprint + Ord,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u64).to_le_bytes());
        for value in self {
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

impl<T> CompatFingerprint for std::collections::VecDeque<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u64).to_le_bytes());
        for value in self {
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

impl<T> CompatFingerprint for std::sync::Arc<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        (**self).compat_fingerprint_with(context)
    }
}

impl<T> CompatFingerprint for std::rc::Rc<T>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        (**self).compat_fingerprint_with(context)
    }
}

impl<T> CompatFingerprint for Box<[T]>
where
    T: CompatFingerprint,
{
    const COMPAT_FINGERPRINT_VERSION: u32 = T::COMPAT_FINGERPRINT_VERSION;

    fn compat_fingerprint(&self) -> Vec<u8> {
        self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
    }

    fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u64).to_le_bytes());
        for value in self.iter() {
            append_fingerprint_part(&mut out, &value.compat_fingerprint_with(context));
        }
        out
    }
}

macro_rules! impl_tuple_fingerprint {
    ($arity:expr, $($index:tt => $name:ident),+ $(,)?) => {
        impl<$($name),+> CompatFingerprint for ($($name,)+)
        where
            $($name: CompatFingerprint),+
        {
            const COMPAT_FINGERPRINT_VERSION: u32 =
                max_fingerprint_versions([$($name::COMPAT_FINGERPRINT_VERSION),+]);

            fn compat_fingerprint(&self) -> Vec<u8> {
                self.compat_fingerprint_with(FingerprintContext::latest::<Self>())
            }

            fn compat_fingerprint_with(&self, context: FingerprintContext) -> Vec<u8> {
                let mut out = Vec::new();
                out.extend_from_slice(&($arity as u64).to_le_bytes());
                $(
                    append_fingerprint_part(&mut out, &self.$index.compat_fingerprint_with(context));
                )+
                out
            }
        }
    };
}

impl_tuple_fingerprint!(1, 0 => A);
impl_tuple_fingerprint!(2, 0 => A, 1 => B);
impl_tuple_fingerprint!(3, 0 => A, 1 => B, 2 => C);
impl_tuple_fingerprint!(4, 0 => A, 1 => B, 2 => C, 3 => D);
impl_tuple_fingerprint!(5, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E);
impl_tuple_fingerprint!(6, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F);
impl_tuple_fingerprint!(7, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G);
impl_tuple_fingerprint!(8, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H);
impl_tuple_fingerprint!(9, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I);
impl_tuple_fingerprint!(10, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I, 9 => J);
impl_tuple_fingerprint!(11, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I, 9 => J, 10 => K);
impl_tuple_fingerprint!(12, 0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I, 9 => J, 10 => K, 11 => L);

/// Computes the compatibility digest using [`DEFAULT_SEED`] and `ChaCha20Rng`.
///
/// # Panics
///
/// Panics if `samples` is zero.
pub fn compat_digest<T>(samples: usize) -> [u8; 32]
where
    T: CompatSampler + CompatSerializer,
{
    let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
    compat_digest_with_rng::<T, _>(&mut rng, samples)
}

/// Computes the compatibility digest using a caller-provided RNG.
///
/// Given `samples = N`, exactly `N` values are sampled and serialized. The
/// first sample is hashed as `SHA256(payload)`. Each following sample is hashed
/// as `SHA256(previous_digest_bytes || payload)`.
///
/// # Panics
///
/// Panics if `samples` is zero.
pub fn compat_digest_with_rng<T, R>(rng: &mut R, samples: usize) -> [u8; 32]
where
    T: CompatSampler + CompatSerializer,
    R: RngCore + ?Sized,
{
    assert!(
        samples > 0,
        "binary_compat sample count must be greater than zero"
    );

    let first = T::compat_sample(rng).compat_serialize();
    let mut digest = first_digest(&first);

    for _ in 1..samples {
        let payload = T::compat_sample(rng).compat_serialize();
        digest = chain_digest(digest, &payload);
    }

    digest
}

/// Computes the digest of a type's serialized shape.
pub fn compat_shape_digest<T>() -> [u8; 32]
where
    T: CompatShape,
{
    first_digest(&T::compat_shape())
}

fn first_digest(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}

fn chain_digest(previous: [u8; 32], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(previous);
    hasher.update(payload);
    hasher.finalize().into()
}

#[cfg(feature = "fixtures")]
fn update_optional_digest(digest: &mut Option<[u8; 32]>, payload: &[u8]) {
    *digest = Some(match *digest {
        Some(previous) => chain_digest(previous, payload),
        None => first_digest(payload),
    });
}

/// Formats a digest as lowercase hexadecimal.
pub fn digest_to_hex(digest: [u8; 32]) -> String {
    hex::encode(digest)
}

/// Computes a serialization compatibility digest and formats it as lowercase hex.
pub fn compat_digest_hex<T>(samples: usize) -> String
where
    T: CompatSampler + CompatSerializer,
{
    digest_to_hex(compat_digest::<T>(samples))
}

/// Computes a shape digest and formats it as lowercase hex.
pub fn compat_shape_digest_hex<T>() -> String
where
    T: CompatShape,
{
    digest_to_hex(compat_shape_digest::<T>())
}

/// Checks a computed digest and returns a detailed mismatch message on failure.
pub fn check_digest(
    type_name: &str,
    expected: [u8; 32],
    actual: [u8; 32],
    samples: usize,
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }

    Err(format!(
        "binary compatibility digest mismatch for {type_name}\n\
         expected: {}\n\
         actual:   {}\n\
         samples:  {samples}\n\
         algorithm: binary_compat digest algorithm v{}, ChaCha20Rng, DEFAULT_SEED",
        digest_to_hex(expected),
        digest_to_hex(actual),
        DIGEST_ALGORITHM_VERSION
    ))
}

/// Checks a computed shape digest and returns a detailed mismatch message on failure.
pub fn check_shape_digest(
    type_name: &str,
    expected: [u8; 32],
    actual: [u8; 32],
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }

    Err(format!(
        "binary compatibility shape digest mismatch for {type_name}\n\
         expected:  {}\n\
         actual:    {}\n\
         algorithm: binary_compat shape digest v{}",
        digest_to_hex(expected),
        digest_to_hex(actual),
        SHAPE_DIGEST_ALGORITHM_VERSION,
    ))
}

/// Human-readable metadata stored in generated deserialization fixtures.
#[cfg(feature = "fixtures")]
#[derive(Debug, Clone, Copy)]
pub struct DeserializeFixtureMetadata<'a> {
    /// Name of the legacy wire format, for example `"bincode standard"`.
    pub format: &'a str,
    /// Producer identity, typically the crate name and version that generated the fixture.
    pub producer: &'a str,
}

#[cfg(feature = "fixtures")]
impl<'a> DeserializeFixtureMetadata<'a> {
    /// Create fixture metadata for a legacy wire format and producer identity.
    pub const fn new(format: &'a str, producer: &'a str) -> Self {
        Self { format, producer }
    }
}

/// Build [`DeserializeFixtureMetadata`] using the current crate name and version.
///
/// The macro expands in the user's crate, so `env!("CARGO_PKG_NAME")` and
/// `env!("CARGO_PKG_VERSION")` describe the crate that invokes it.
#[cfg(feature = "fixtures")]
#[macro_export]
macro_rules! deserialize_fixture_metadata {
    ($format:expr $(,)?) => {
        $crate::DeserializeFixtureMetadata::new(
            $format,
            concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION")),
        )
    };
    ($format:expr, producer = $producer:expr $(,)?) => {
        $crate::DeserializeFixtureMetadata::new($format, $producer)
    };
}

/// Errors produced while writing or checking deserialization fixtures.
#[cfg(feature = "fixtures")]
#[derive(Debug)]
pub enum FixtureError {
    /// Fixture generation or validation was requested with zero samples.
    InvalidSamples,
    /// The JSON `samples` field does not match the number of payloads.
    SampleCountMismatch { declared: usize, actual: usize },
    /// The fixture format version is not supported.
    UnsupportedVersion { version: u32 },
    /// The semantic digest algorithm version is not supported.
    UnsupportedDigestAlgorithm { version: u32 },
    /// Writing the fixture file failed.
    Io(std::io::Error),
    /// Parsing or formatting fixture JSON failed.
    Json(serde_json::Error),
    /// One payload entry was not valid hex.
    InvalidPayloadHex {
        index: usize,
        error: hex::FromHexError,
    },
    /// The stored payload digest does not match the payload list.
    PayloadDigestMismatch { expected: String, actual: String },
    /// A payload could not be decoded by the current deserializer.
    Decode {
        type_name: &'static str,
        index: usize,
        error: String,
    },
    /// Payloads decoded successfully, but not to the expected semantic values.
    SemanticDigestMismatch {
        type_name: &'static str,
        expected: String,
        actual: String,
        samples: usize,
    },
}

#[cfg(feature = "fixtures")]
impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSamples => {
                write!(
                    f,
                    "deserialization fixture sample count must be greater than zero"
                )
            }
            Self::SampleCountMismatch { declared, actual } => write!(
                f,
                "deserialization fixture declares {declared} samples but contains {actual} payloads"
            ),
            Self::UnsupportedVersion { version } => {
                write!(f, "unsupported deserialization fixture version {version}")
            }
            Self::UnsupportedDigestAlgorithm { version } => write!(
                f,
                "unsupported deserialization fixture digest algorithm version {version}"
            ),
            Self::Io(error) => write!(f, "failed to write deserialization fixture: {error}"),
            Self::Json(error) => write!(f, "failed to parse deserialization fixture JSON: {error}"),
            Self::InvalidPayloadHex { index, error } => {
                write!(f, "payload {index} is not valid hex: {error}")
            }
            Self::PayloadDigestMismatch { expected, actual } => write!(
                f,
                "deserialization fixture payload digest mismatch\nexpected: {expected}\nactual:   {actual}"
            ),
            Self::Decode {
                type_name,
                index,
                error,
            } => write!(
                f,
                "failed to deserialize fixture payload {index} for {type_name}: {error}"
            ),
            Self::SemanticDigestMismatch {
                type_name,
                expected,
                actual,
                samples,
            } => write!(
                f,
                "deserialization fixture semantic digest mismatch for {type_name}\nexpected: {expected}\nactual:   {actual}\nsamples:  {samples}"
            ),
        }
    }
}

#[cfg(feature = "fixtures")]
impl std::error::Error for FixtureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidPayloadHex { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "fixtures")]
impl From<std::io::Error> for FixtureError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(feature = "fixtures")]
impl From<serde_json::Error> for FixtureError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(feature = "fixtures")]
#[derive(serde::Deserialize, serde::Serialize)]
struct DeserializeFixture {
    version: u32,
    type_name: String,
    format: String,
    producer: String,
    samples: usize,
    seed: String,
    digest_algorithm_version: u32,
    #[serde(default = "default_fingerprint_version")]
    fingerprint_version: u32,
    semantic_digest: String,
    payload_digest: String,
    payloads: Vec<String>,
}

#[cfg(feature = "fixtures")]
const DESERIALIZE_FIXTURE_VERSION: u32 = 1;
#[cfg(feature = "fixtures")]
const DEFAULT_SEED_NAME: &str = "binary_compat default seed v1";

#[cfg(feature = "fixtures")]
const fn default_fingerprint_version() -> u32 {
    1
}

/// Generate a JSON fixture containing old wire payloads and their semantic digest.
#[cfg(feature = "fixtures")]
pub fn write_deserialize_fixture<T>(
    path: impl AsRef<std::path::Path>,
    samples: usize,
    metadata: DeserializeFixtureMetadata<'_>,
) -> Result<(), FixtureError>
where
    T: CompatSampler + CompatSerializer + CompatFingerprint,
{
    if samples == 0 {
        return Err(FixtureError::InvalidSamples);
    }

    let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
    let mut semantic_digest = None;
    let mut payload_digest = None;
    let mut payloads = Vec::with_capacity(samples);
    let fingerprint_context = FingerprintContext::latest::<T>();

    for _ in 0..samples {
        let value = T::compat_sample(&mut rng);
        let payload = value.compat_serialize();
        let fingerprint = value.compat_fingerprint_with(fingerprint_context);

        update_optional_digest(&mut semantic_digest, &fingerprint);
        update_optional_digest(&mut payload_digest, &payload);
        payloads.push(hex::encode(payload));
    }

    let fixture = DeserializeFixture {
        version: DESERIALIZE_FIXTURE_VERSION,
        type_name: std::any::type_name::<T>().to_owned(),
        format: metadata.format.to_owned(),
        producer: metadata.producer.to_owned(),
        samples,
        seed: DEFAULT_SEED_NAME.to_owned(),
        digest_algorithm_version: DIGEST_ALGORITHM_VERSION,
        fingerprint_version: T::COMPAT_FINGERPRINT_VERSION,
        semantic_digest: digest_to_hex(semantic_digest.expect("samples is non-zero")),
        payload_digest: digest_to_hex(payload_digest.expect("samples is non-zero")),
        payloads,
    };

    let mut json = serde_json::to_string_pretty(&fixture)?;
    json.push('\n');
    std::fs::write(path, json)?;
    Ok(())
}

/// Generate a JSON fixture using [`DEFAULT_SAMPLES`].
#[cfg(feature = "fixtures")]
pub fn write_default_deserialize_fixture<T>(
    path: impl AsRef<std::path::Path>,
    metadata: DeserializeFixtureMetadata<'_>,
) -> Result<(), FixtureError>
where
    T: CompatSampler + CompatSerializer + CompatFingerprint,
{
    write_deserialize_fixture::<T>(path, DEFAULT_SAMPLES, metadata)
}

/// Assert that the current deserializer can read a previously generated fixture.
///
/// Validation is by content, not by identity: the fixture's `version`,
/// `digest_algorithm_version`, `samples`, `payload_digest`, and
/// `semantic_digest` are all checked, but the human-readable metadata fields
/// (`type_name`, `format`, `producer`, `seed`) are treated as informational
/// only. A fixture generated for a type that has since been renamed will
/// still load successfully so long as its bytes decode and the resulting
/// fingerprints match what was stored. Use `#[compat(fingerprint_with = ...)]`
/// or a custom semantic fingerprint to enforce stricter identity checks.
#[cfg(feature = "fixtures")]
pub fn assert_deserialize_fixture<T>(fixture_json: &str) -> Result<(), FixtureError>
where
    T: CompatDeserializer + CompatFingerprint,
{
    let fixture: DeserializeFixture = serde_json::from_str(fixture_json)?;

    if fixture.version != DESERIALIZE_FIXTURE_VERSION {
        return Err(FixtureError::UnsupportedVersion {
            version: fixture.version,
        });
    }

    if fixture.digest_algorithm_version != DIGEST_ALGORITHM_VERSION {
        return Err(FixtureError::UnsupportedDigestAlgorithm {
            version: fixture.digest_algorithm_version,
        });
    }

    if fixture.samples == 0 {
        return Err(FixtureError::InvalidSamples);
    }

    if fixture.samples != fixture.payloads.len() {
        return Err(FixtureError::SampleCountMismatch {
            declared: fixture.samples,
            actual: fixture.payloads.len(),
        });
    }

    let mut payload_digest = None;
    let mut payloads = Vec::with_capacity(fixture.payloads.len());

    for (index, payload_hex) in fixture.payloads.iter().enumerate() {
        let payload = hex::decode(payload_hex)
            .map_err(|error| FixtureError::InvalidPayloadHex { index, error })?;
        update_optional_digest(&mut payload_digest, &payload);
        payloads.push(payload);
    }

    let actual_payload_digest = digest_to_hex(payload_digest.expect("samples is non-zero"));
    if fixture.payload_digest != actual_payload_digest {
        return Err(FixtureError::PayloadDigestMismatch {
            expected: fixture.payload_digest,
            actual: actual_payload_digest,
        });
    }

    let mut semantic_digest = None;
    let fingerprint_context = FingerprintContext::new(fixture.fingerprint_version);
    for (index, payload) in payloads.iter().enumerate() {
        let decoded = T::compat_deserialize(payload).map_err(|error| FixtureError::Decode {
            type_name: std::any::type_name::<T>(),
            index,
            error: format!("{error:?}"),
        })?;
        update_optional_digest(
            &mut semantic_digest,
            &decoded.compat_fingerprint_with(fingerprint_context),
        );
    }

    let actual_semantic_digest = digest_to_hex(semantic_digest.expect("samples is non-zero"));
    if fixture.semantic_digest != actual_semantic_digest {
        return Err(FixtureError::SemanticDigestMismatch {
            type_name: std::any::type_name::<T>(),
            expected: fixture.semantic_digest,
            actual: actual_semantic_digest,
            samples: fixture.samples,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CounterSample(u32);

    impl CompatSampler for CounterSample {
        fn compat_sample<R>(rng: &mut R) -> Self
        where
            R: RngCore + ?Sized,
        {
            Self(rng.next_u32())
        }
    }

    impl CompatSerializer for CounterSample {
        fn compat_serialize(&self) -> Vec<u8> {
            self.0.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn sample_one_hashes_only_the_payload() {
        let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
        let payload = CounterSample::compat_sample(&mut rng).compat_serialize();
        let expected: [u8; 32] = Sha256::digest(payload).into();

        assert_eq!(compat_digest::<CounterSample>(1), expected);
    }

    #[test]
    fn digest_is_stable_for_a_toy_type() {
        assert_eq!(
            digest_to_hex(compat_digest::<CounterSample>(3)),
            "1868545fe53a773e1315d3d551f9187e87b46e084f6aa38fb09cfca6b7cd99af"
        );
        assert_eq!(
            compat_digest_hex::<CounterSample>(3),
            "1868545fe53a773e1315d3d551f9187e87b46e084f6aa38fb09cfca6b7cd99af"
        );
    }

    #[test]
    #[should_panic(expected = "sample count must be greater than zero")]
    fn zero_samples_panic() {
        let _ = compat_digest::<CounterSample>(0);
    }

    #[test]
    fn check_digest_reports_context() {
        let message = check_digest("CounterSample", [0; 32], [1; 32], 5).unwrap_err();

        assert!(message.contains("CounterSample"));
        assert!(message.contains("samples:  5"));
        assert!(message.contains("ChaCha20Rng"));
    }

    #[test]
    fn sampler_and_fingerprint_cover_tuples_up_to_arity_twelve() {
        fn assert_sample_and_fingerprint<T: CompatSampler + CompatFingerprint>() {
            let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
            let _ = T::compat_sample(&mut rng).compat_fingerprint();
        }

        assert_sample_and_fingerprint::<(u8, u8, u8, u8, u8, u8, u8, u8, u8)>();
        assert_sample_and_fingerprint::<(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)>();
        assert_sample_and_fingerprint::<(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)>();
        assert_sample_and_fingerprint::<(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)>();
    }

    #[test]
    fn sampler_and_fingerprint_cover_arc_rc_vecdeque_and_box_slice() {
        fn assert_sample_and_fingerprint<T: CompatSampler + CompatFingerprint>() {
            let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
            let _ = T::compat_sample(&mut rng).compat_fingerprint();
        }

        assert_sample_and_fingerprint::<std::sync::Arc<u32>>();
        assert_sample_and_fingerprint::<std::rc::Rc<u32>>();
        assert_sample_and_fingerprint::<std::collections::VecDeque<u32>>();
        assert_sample_and_fingerprint::<Box<[u32]>>();
    }

    #[test]
    fn usize_sampler_truncates_to_low_32_bits_for_cross_platform_stability() {
        let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
        let first = usize::compat_sample(&mut rng);
        let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
        let reference = rng.next_u32();

        assert_eq!(first as u32, reference);
        assert!(first <= u32::MAX as usize);

        let mut rng = ChaCha20Rng::from_seed(DEFAULT_SEED);
        let signed = isize::compat_sample(&mut rng);
        assert_eq!(signed as i32 as u32, reference);
    }

    #[test]
    fn shape_supports_solana_frozen_abi_standard_types() {
        fn assert_shape<T: CompatShape>() {
            assert_ne!(compat_shape_digest::<T>(), [0; 32]);
        }

        assert_shape::<(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)>();
        assert_shape::<std::time::Duration>();
        assert_shape::<std::time::SystemTime>();
        assert_shape::<std::sync::Once>();
        assert_shape::<std::sync::OnceLock<u8>>();
        assert_shape::<std::sync::atomic::AtomicU8>();
        assert_shape::<std::sync::atomic::AtomicU16>();
        assert_shape::<std::sync::atomic::AtomicU32>();
        assert_shape::<std::sync::atomic::AtomicU64>();
        assert_shape::<std::sync::atomic::AtomicUsize>();
        assert_shape::<std::sync::atomic::AtomicI8>();
        assert_shape::<std::sync::atomic::AtomicI16>();
        assert_shape::<std::sync::atomic::AtomicI32>();
        assert_shape::<std::sync::atomic::AtomicI64>();
        assert_shape::<std::sync::atomic::AtomicIsize>();
        assert_shape::<std::sync::atomic::AtomicBool>();
        assert_shape::<Box<[u8]>>();
        assert_shape::<Box<dyn Fn(&mut u8) + Send + Sync>>();
        assert_shape::<Box<dyn Fn(&mut u8, u16) + Send + Sync>>();
        assert_shape::<std::sync::Arc<u8>>();
        assert_shape::<&'static u8>();
        assert_shape::<&'static [u8]>();
        assert_shape::<std::sync::Weak<u8>>();
        assert_shape::<std::rc::Rc<u8>>();
        assert_shape::<std::rc::Weak<u8>>();
        assert_shape::<std::sync::Mutex<u8>>();
        assert_shape::<std::sync::RwLock<u8>>();
        assert_shape::<std::collections::HashMap<u8, u16>>();
        assert_shape::<std::collections::HashSet<u8>>();
        assert_shape::<std::collections::VecDeque<u8>>();
        assert_shape::<std::path::PathBuf>();
        assert_shape::<std::net::SocketAddr>();
        assert_shape::<std::net::IpAddr>();
    }
}
