#[cfg(feature = "macros")]
mod macros {
    use binary_compat::CompatFingerprint as _;

    #[cfg_attr(
        feature = "macros",
        binary_compat::compat_test(
            digest = "1868545fe53a773e1315d3d551f9187e87b46e084f6aa38fb09cfca6b7cd99af",
            samples = 3,
        )
    )]
    struct CounterSample(u32);

    impl binary_compat::CompatSampler for CounterSample {
        fn compat_sample<R>(rng: &mut R) -> Self
        where
            R: binary_compat::RngCore + ?Sized,
        {
            Self(rng.next_u32())
        }
    }

    impl binary_compat::CompatSerializer for CounterSample {
        fn compat_serialize(&self) -> Vec<u8> {
            self.0.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn macro_feature_is_enabled_for_this_test() {
        assert_eq!(
            binary_compat::digest_to_hex(binary_compat::compat_digest::<CounterSample>(3)),
            "1868545fe53a773e1315d3d551f9187e87b46e084f6aa38fb09cfca6b7cd99af"
        );
    }

    #[cfg_attr(
        feature = "macros",
        binary_compat::compat_test(
            digest = "2af9f2a672bec85e6655ec4569991a5f27ae4f0f0011f722aa13c44a1b962eb5",
            shape_digest = "4b4eca48624c1f60e11ca3c70d3285aff901af2760f0ec1ffdb82effec356963",
            samples = 3,
        )
    )]
    #[derive(binary_compat::CompatSampler, binary_compat::CompatShape)]
    struct ShapeChecked {
        id: u32,
        flag: bool,
    }

    impl binary_compat::CompatSerializer for ShapeChecked {
        fn compat_serialize(&self) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&self.id.to_le_bytes());
            out.push(u8::from(self.flag));
            out
        }
    }

    #[test]
    fn shape_and_serialization_are_distinct_digests() {
        let serialization_digest = binary_compat::compat_digest::<ShapeChecked>(3);
        let shape_digest = binary_compat::compat_shape_digest::<ShapeChecked>();

        assert_ne!(serialization_digest, shape_digest);
    }

    #[cfg_attr(
        feature = "macros",
        binary_compat::compat_test(
            digest = "223fd96bbfa8b5299cd30d50c3d05240677a18fa991056656de6ec0e0d3830ef",
            shape_digest = "b339bd545b913660083d90460fea6b38defdbd63b526a4b888c8215f2bc95aee",
            samples = 4,
        )
    )]
    #[derive(binary_compat::CompatShape)]
    enum EnumChecked {
        Unit,
        Tuple(u8),
        Struct { flag: bool },
    }

    impl binary_compat::CompatSampler for EnumChecked {
        fn compat_sample<R>(rng: &mut R) -> Self
        where
            R: binary_compat::RngCore + ?Sized,
        {
            match rng.next_u32() % 3 {
                0 => Self::Unit,
                1 => Self::Tuple(rng.next_u32() as u8),
                _ => Self::Struct {
                    flag: rng.next_u32() & 1 == 1,
                },
            }
        }
    }

    impl binary_compat::CompatSerializer for EnumChecked {
        fn compat_serialize(&self) -> Vec<u8> {
            match self {
                Self::Unit => vec![0],
                Self::Tuple(value) => vec![1, *value],
                Self::Struct { flag } => vec![2, u8::from(*flag)],
            }
        }
    }

    #[derive(binary_compat::CompatSampler)]
    struct DerivedSample<T> {
        id: u32,
        label: String,
        flags: [bool; 3],
        values: Vec<Option<T>>,
    }

    impl<T> binary_compat::CompatSerializer for DerivedSample<T>
    where
        T: binary_compat::CompatSerializer,
    {
        fn compat_serialize(&self) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&self.id.to_le_bytes());
            out.extend_from_slice(&(self.label.len() as u32).to_le_bytes());
            out.extend_from_slice(self.label.as_bytes());
            out.extend(self.flags.iter().map(|flag| u8::from(*flag)));
            out.extend_from_slice(&(self.values.len() as u32).to_le_bytes());
            for value in &self.values {
                match value {
                    Some(value) => {
                        out.push(1);
                        out.extend(value.compat_serialize());
                    }
                    None => out.push(0),
                }
            }
            out
        }
    }

    #[derive(Debug, Eq, PartialEq, binary_compat::CompatSampler, binary_compat::CompatShape)]
    #[cfg_attr(feature = "macros", derive(binary_compat::CompatFingerprint))]
    struct Leaf(u16);

    impl binary_compat::CompatSerializer for Leaf {
        fn compat_serialize(&self) -> Vec<u8> {
            self.0.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn derive_sampler_uses_field_samplers() {
        assert_eq!(
            binary_compat::compat_digest::<DerivedSample<Leaf>>(2),
            binary_compat::compat_digest::<DerivedSample<Leaf>>(2)
        );
    }

    mod external {
        #[derive(Debug, Eq, PartialEq)]
        pub struct Foreign(pub u8);

        #[derive(Debug, Eq, PartialEq)]
        pub struct DefaultOnly(pub u8);

        impl Default for DefaultOnly {
            fn default() -> Self {
                Self(7)
            }
        }
    }

    fn sample_foreign<R>(rng: &mut R) -> external::Foreign
    where
        R: binary_compat::RngCore + ?Sized,
    {
        external::Foreign((rng.next_u32() % 251) as u8)
    }

    #[derive(binary_compat::CompatSampler)]
    #[compat(crate = binary_compat)]
    struct UsesForeignFields {
        #[compat(sample_with = sample_foreign)]
        sampled: external::Foreign,
        #[compat(default)]
        defaulted: external::DefaultOnly,
        #[compat(value = external::Foreign(42))]
        fixed: external::Foreign,
    }

    struct CountingRng(u64);

    impl binary_compat::RngCore for CountingRng {
        fn next_u32(&mut self) -> u32 {
            self.0 += 1;
            self.0 as u32
        }

        fn next_u64(&mut self) -> u64 {
            let high = self.next_u32() as u64;
            let low = self.next_u32() as u64;
            (high << 32) | low
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(4) {
                let bytes = self.next_u32().to_le_bytes();
                let len = chunk.len();
                chunk.copy_from_slice(&bytes[..len]);
            }
        }
    }

    #[test]
    fn derive_sampler_supports_field_overrides() {
        let mut rng = CountingRng(0);
        let sample = <UsesForeignFields as binary_compat::CompatSampler>::compat_sample(&mut rng);

        assert_eq!(sample.sampled, external::Foreign(1));
        assert_eq!(sample.defaulted, external::DefaultOnly(7));
        assert_eq!(sample.fixed, external::Foreign(42));
    }

    fn fingerprint_foreign(value: &external::Foreign) -> Vec<u8> {
        vec![value.0]
    }

    #[derive(binary_compat::CompatFingerprint, binary_compat::CompatSampler)]
    #[compat(crate = binary_compat)]
    #[allow(dead_code)]
    struct FingerprintedForeignFields<T> {
        id: u8,
        value: T,
        #[compat(sample_with = sample_foreign, fingerprint_with = fingerprint_foreign)]
        external: external::Foreign,
        #[compat(fingerprint_since = 2)]
        added: Option<u8>,
        #[compat(default, fingerprint_skip)]
        cache: external::DefaultOnly,
    }

    #[derive(binary_compat::CompatFingerprint)]
    enum FingerprintEnum<T> {
        Unit,
        Tuple(u8, T),
        Struct { flag: bool },
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, binary_compat::CompatFingerprint)]
    struct VersionedFingerprintLeaf {
        id: u8,
        #[compat(fingerprint_since = 2)]
        added: u8,
    }

    #[derive(binary_compat::CompatFingerprint)]
    struct NestedVersionedFingerprint {
        maybe: Option<VersionedFingerprintLeaf>,
        list: Vec<VersionedFingerprintLeaf>,
        tuple: (VersionedFingerprintLeaf, Option<VersionedFingerprintLeaf>),
    }

    #[test]
    fn derive_fingerprint_supports_generics_enums_and_overrides() {
        let value = FingerprintedForeignFields {
            id: 9,
            value: Leaf(12),
            external: external::Foreign(33),
            added: Some(5),
            cache: external::DefaultOnly(7),
        };

        let fingerprint = value.compat_fingerprint();
        assert!(fingerprint.windows(1).any(|bytes| bytes == [33]));
        assert_eq!(
            <FingerprintedForeignFields<Leaf> as binary_compat::CompatFingerprint>::COMPAT_FINGERPRINT_VERSION,
            2
        );
        assert_eq!(
            value.compat_fingerprint_with(binary_compat::FingerprintContext::new(1)),
            FingerprintedForeignFields {
                id: 9,
                value: Leaf(12),
                external: external::Foreign(33),
                added: None,
                cache: external::DefaultOnly(7),
            }
            .compat_fingerprint_with(binary_compat::FingerprintContext::new(1))
        );
        assert_ne!(
            value.compat_fingerprint_with(binary_compat::FingerprintContext::new(2)),
            FingerprintedForeignFields {
                id: 9,
                value: Leaf(12),
                external: external::Foreign(33),
                added: None,
                cache: external::DefaultOnly(7),
            }
            .compat_fingerprint_with(binary_compat::FingerprintContext::new(2))
        );
        assert_eq!(
            value.compat_fingerprint(),
            FingerprintedForeignFields {
                cache: external::DefaultOnly(99),
                ..value
            }
            .compat_fingerprint()
        );

        assert_ne!(
            FingerprintEnum::<Leaf>::Unit.compat_fingerprint(),
            FingerprintEnum::Tuple(0, Leaf(0)).compat_fingerprint()
        );
        assert_ne!(
            FingerprintEnum::<Leaf>::Tuple(0, Leaf(0)).compat_fingerprint(),
            (FingerprintEnum::<Leaf>::Struct { flag: false }).compat_fingerprint()
        );
    }

    fn assert_context_skips_new_field<T>(left: &T, right: &T)
    where
        T: binary_compat::CompatFingerprint,
    {
        assert_eq!(T::COMPAT_FINGERPRINT_VERSION, 2);
        assert_eq!(
            left.compat_fingerprint_with(binary_compat::FingerprintContext::new(1)),
            right.compat_fingerprint_with(binary_compat::FingerprintContext::new(1))
        );
        assert_ne!(
            left.compat_fingerprint_with(binary_compat::FingerprintContext::new(2)),
            right.compat_fingerprint_with(binary_compat::FingerprintContext::new(2))
        );
    }

    #[test]
    fn derived_fingerprint_propagates_context_through_nested_fields() {
        let old_leaf = VersionedFingerprintLeaf { id: 7, added: 1 };
        let new_leaf = VersionedFingerprintLeaf { id: 7, added: 2 };

        assert_context_skips_new_field(
            &NestedVersionedFingerprint {
                maybe: Some(old_leaf.clone()),
                list: vec![old_leaf.clone()],
                tuple: (old_leaf.clone(), Some(old_leaf)),
            },
            &NestedVersionedFingerprint {
                maybe: Some(new_leaf.clone()),
                list: vec![new_leaf.clone()],
                tuple: (new_leaf.clone(), Some(new_leaf)),
            },
        );
    }

    #[test]
    fn built_in_fingerprint_impls_propagate_context_to_nested_values() {
        let old_leaf = VersionedFingerprintLeaf { id: 7, added: 1 };
        let new_leaf = VersionedFingerprintLeaf { id: 7, added: 2 };

        assert_context_skips_new_field(&Some(old_leaf.clone()), &Some(new_leaf.clone()));
        assert_context_skips_new_field(
            &Result::<_, u8>::Ok(old_leaf.clone()),
            &Result::<_, u8>::Ok(new_leaf.clone()),
        );
        assert_context_skips_new_field(&vec![old_leaf.clone()], &vec![new_leaf.clone()]);
        assert_context_skips_new_field(&Box::new(old_leaf.clone()), &Box::new(new_leaf.clone()));
        assert_context_skips_new_field(
            &[old_leaf.clone(), old_leaf.clone()],
            &[new_leaf.clone(), new_leaf.clone()],
        );
        assert_context_skips_new_field(
            &std::sync::Arc::new(old_leaf.clone()),
            &std::sync::Arc::new(new_leaf.clone()),
        );
        assert_context_skips_new_field(
            &std::rc::Rc::new(old_leaf.clone()),
            &std::rc::Rc::new(new_leaf.clone()),
        );
        assert_context_skips_new_field(
            &Box::<[VersionedFingerprintLeaf]>::from([old_leaf.clone()]),
            &Box::<[VersionedFingerprintLeaf]>::from([new_leaf.clone()]),
        );
        assert_context_skips_new_field(
            &std::collections::VecDeque::from([old_leaf.clone()]),
            &std::collections::VecDeque::from([new_leaf.clone()]),
        );
        assert_context_skips_new_field(
            &(old_leaf.clone(), Some(old_leaf.clone())),
            &(new_leaf.clone(), Some(new_leaf.clone())),
        );

        let old_map = std::collections::BTreeMap::from([(old_leaf.clone(), 11_u8)]);
        let new_map = std::collections::BTreeMap::from([(new_leaf.clone(), 11_u8)]);
        assert_context_skips_new_field(&old_map, &new_map);

        let old_set = std::collections::BTreeSet::from([old_leaf]);
        let new_set = std::collections::BTreeSet::from([new_leaf]);
        assert_context_skips_new_field(&old_set, &new_set);
    }

    fn shape_foreign() -> Vec<u8> {
        let mut out = Vec::new();
        binary_compat::append_shape_part(&mut out, b"foreign");
        binary_compat::append_shape_part(&mut out, b"external::Foreign");
        out
    }

    #[derive(binary_compat::CompatShape)]
    #[compat(crate = binary_compat)]
    #[allow(dead_code)]
    struct ShapeWithForeign<T> {
        id: u8,
        value: T,
        #[compat(shape_with = shape_foreign)]
        external: external::Foreign,
        #[compat(shape_skip)]
        cache: external::DefaultOnly,
    }

    #[derive(binary_compat::CompatShape)]
    struct SameBytesOriginal {
        id: u32,
    }

    #[derive(binary_compat::CompatShape)]
    struct SameBytesRenamed {
        renamed_id: u32,
    }

    #[derive(binary_compat::CompatShape)]
    #[allow(dead_code)]
    enum ShapeEnum<T> {
        Unit,
        Tuple(u8, T),
        Struct { flag: bool },
    }

    #[test]
    fn derive_shape_supports_structs_enums_generics_and_overrides() {
        let original_shape = binary_compat::compat_shape_digest::<SameBytesOriginal>();
        let renamed_shape = binary_compat::compat_shape_digest::<SameBytesRenamed>();
        assert_ne!(original_shape, renamed_shape);

        assert_eq!(
            binary_compat::compat_shape_digest::<ShapeWithForeign<Leaf>>(),
            binary_compat::compat_shape_digest::<ShapeWithForeign<Leaf>>()
        );
        assert_ne!(
            binary_compat::compat_shape_digest::<ShapeEnum<Leaf>>(),
            binary_compat::compat_shape_digest::<SameBytesOriginal>()
        );
    }

    impl binary_compat::CompatSampler for SameBytesOriginal {
        fn compat_sample<R>(rng: &mut R) -> Self
        where
            R: binary_compat::RngCore + ?Sized,
        {
            Self { id: rng.next_u32() }
        }
    }

    impl binary_compat::CompatSerializer for SameBytesOriginal {
        fn compat_serialize(&self) -> Vec<u8> {
            self.id.to_le_bytes().to_vec()
        }
    }

    impl binary_compat::CompatSampler for SameBytesRenamed {
        fn compat_sample<R>(rng: &mut R) -> Self
        where
            R: binary_compat::RngCore + ?Sized,
        {
            Self {
                renamed_id: rng.next_u32(),
            }
        }
    }

    impl binary_compat::CompatSerializer for SameBytesRenamed {
        fn compat_serialize(&self) -> Vec<u8> {
            self.renamed_id.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn shape_digest_changes_when_shape_changes_but_bytes_do_not() {
        assert_eq!(
            binary_compat::compat_digest::<SameBytesOriginal>(3),
            binary_compat::compat_digest::<SameBytesRenamed>(3)
        );
        assert_ne!(
            binary_compat::compat_shape_digest::<SameBytesOriginal>(),
            binary_compat::compat_shape_digest::<SameBytesRenamed>()
        );
    }

    #[derive(Debug, Eq, PartialEq, binary_compat::CompatSampler)]
    enum DerivedEnum<T> {
        Unit,
        Tuple(u8, T),
        Struct {
            flag: bool,
            #[compat(sample_with = sample_foreign)]
            external: external::Foreign,
        },
    }

    #[test]
    fn derive_sampler_supports_enums() {
        let mut unit_rng = CountingRng(0);
        assert!(matches!(
            <DerivedEnum<Leaf> as binary_compat::CompatSampler>::compat_sample(&mut unit_rng),
            DerivedEnum::Unit
        ));

        let mut tuple_rng = CountingRng(2);
        assert!(matches!(
            <DerivedEnum<Leaf> as binary_compat::CompatSampler>::compat_sample(&mut tuple_rng),
            DerivedEnum::Tuple(_, _)
        ));

        let mut struct_rng = CountingRng(1);
        match <DerivedEnum<Leaf> as binary_compat::CompatSampler>::compat_sample(&mut struct_rng) {
            DerivedEnum::Struct { external, .. } => {
                assert_eq!(external, external::Foreign(5));
            }
            sample => panic!("expected struct variant, got {sample:?}"),
        }
    }

    #[cfg(any(feature = "bincode", feature = "bincode2"))]
    mod bincode_serializer {
        #[derive(
            Debug,
            PartialEq,
            binary_compat::BincodeSerializer,
            binary_compat::BincodeDeserializer,
            bincode::Encode,
            bincode::Decode,
        )]
        #[compat(bincode = "2")]
        struct Encoded {
            id: u32,
            flag: bool,
        }

        #[cfg(not(feature = "bincode1"))]
        #[derive(
            Debug,
            PartialEq,
            binary_compat::BincodeSerializer,
            binary_compat::BincodeDeserializer,
            bincode::Encode,
            bincode::Decode,
        )]
        struct AutoEncoded {
            id: u32,
            flag: bool,
        }

        #[test]
        fn bincode_serializer_matches_bincode_output() {
            let value = Encoded { id: 42, flag: true };

            assert_eq!(
                binary_compat::CompatSerializer::compat_serialize(&value),
                bincode::encode_to_vec(&value, bincode::config::standard()).unwrap()
            );
        }

        #[test]
        fn bincode_deserializer_matches_bincode_output() {
            let value = Encoded { id: 42, flag: true };
            let bytes = bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();

            assert_eq!(
                <Encoded as binary_compat::CompatDeserializer>::compat_deserialize(&bytes).unwrap(),
                value
            );

            let mut bytes_with_trailing = bytes;
            bytes_with_trailing.push(0);
            assert!(
                <Encoded as binary_compat::CompatDeserializer>::compat_deserialize(
                    &bytes_with_trailing
                )
                .is_err()
            );
        }

        #[cfg(not(feature = "bincode1"))]
        #[test]
        fn bincode2_auto_serializer_and_deserializer_use_bincode2() {
            let value = AutoEncoded { id: 42, flag: true };
            let bytes = bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();

            assert_eq!(
                binary_compat::CompatSerializer::compat_serialize(&value),
                bytes
            );
            assert_eq!(
                <AutoEncoded as binary_compat::CompatDeserializer>::compat_deserialize(&bytes)
                    .unwrap(),
                value
            );
        }
    }

    #[cfg(feature = "bincode1")]
    mod bincode1_serializer {
        #[derive(
            Debug,
            PartialEq,
            serde::Serialize,
            serde::Deserialize,
            binary_compat::BincodeSerializer,
            binary_compat::BincodeDeserializer,
        )]
        #[compat(bincode = "1")]
        struct SerdeEncoded {
            id: u32,
            flag: bool,
        }

        #[cfg(not(feature = "bincode2"))]
        #[derive(
            Debug,
            PartialEq,
            serde::Serialize,
            serde::Deserialize,
            binary_compat::BincodeSerializer,
            binary_compat::BincodeDeserializer,
        )]
        struct AutoSerdeEncoded {
            id: u32,
            flag: bool,
        }

        #[test]
        fn bincode1_serializer_matches_bincode1_output() {
            let value = SerdeEncoded { id: 42, flag: true };

            assert_eq!(
                binary_compat::CompatSerializer::compat_serialize(&value),
                binary_compat::__private::bincode1::serialize(&value).unwrap()
            );
        }

        #[test]
        fn bincode1_deserializer_matches_bincode1_output() {
            let value = SerdeEncoded { id: 42, flag: true };
            let bytes = binary_compat::__private::bincode1::serialize(&value).unwrap();

            assert_eq!(
                <SerdeEncoded as binary_compat::CompatDeserializer>::compat_deserialize(&bytes)
                    .unwrap(),
                value
            );

            let mut bytes_with_trailing = bytes;
            bytes_with_trailing.push(0);
            assert!(
                <SerdeEncoded as binary_compat::CompatDeserializer>::compat_deserialize(
                    &bytes_with_trailing
                )
                .is_err()
            );
        }

        #[cfg(not(feature = "bincode2"))]
        #[test]
        fn bincode1_auto_serializer_and_deserializer_use_bincode1() {
            let value = AutoSerdeEncoded { id: 42, flag: true };
            let bytes = binary_compat::__private::bincode1::serialize(&value).unwrap();

            assert_eq!(
                binary_compat::CompatSerializer::compat_serialize(&value),
                bytes
            );
            assert_eq!(
                <AutoSerdeEncoded as binary_compat::CompatDeserializer>::compat_deserialize(&bytes)
                    .unwrap(),
                value
            );
        }
    }

    #[cfg(feature = "wincode")]
    mod wincode_serializer {
        #[derive(
            Debug,
            PartialEq,
            binary_compat::WincodeSerializer,
            binary_compat::WincodeDeserializer,
            wincode::SchemaWrite,
            wincode::SchemaRead,
        )]
        struct Encoded {
            id: u32,
            flag: bool,
        }

        #[test]
        fn wincode_serializer_matches_wincode_output() {
            let value = Encoded { id: 42, flag: true };

            assert_eq!(
                binary_compat::CompatSerializer::compat_serialize(&value),
                wincode::serialize(&value).unwrap()
            );
        }

        #[test]
        fn wincode_deserializer_matches_wincode_output() {
            let value = Encoded { id: 42, flag: true };
            let bytes = wincode::serialize(&value).unwrap();

            assert_eq!(
                <Encoded as binary_compat::CompatDeserializer>::compat_deserialize(&bytes).unwrap(),
                value
            );

            let mut bytes_with_trailing = bytes;
            bytes_with_trailing.push(0);
            assert!(
                <Encoded as binary_compat::CompatDeserializer>::compat_deserialize(
                    &bytes_with_trailing
                )
                .is_err()
            );
        }
    }

    #[cfg(feature = "fixtures")]
    mod fixtures {
        use sha2::{Digest, Sha256};

        #[binary_compat::compat_deserialize_test(fixture = "fixtures/manual_fixture.json")]
        struct ManualFixture(u8);

        impl binary_compat::CompatDeserializer for ManualFixture {
            type Error = &'static str;

            fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
                match bytes {
                    [value] => Ok(Self(*value)),
                    _ => Err("expected one byte"),
                }
            }
        }

        impl binary_compat::CompatFingerprint for ManualFixture {
            fn compat_fingerprint(&self) -> Vec<u8> {
                vec![self.0]
            }
        }

        #[binary_compat::compat_deserialize_test(fixtures(
            legacy_v1 = "fixtures/manual_fixture.json",
            legacy_v2 = "fixtures/manual_fixture_v2.json",
        ))]
        struct MultiManualFixture(u8);

        impl binary_compat::CompatDeserializer for MultiManualFixture {
            type Error = &'static str;

            fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
                match bytes {
                    [value] => Ok(Self(*value)),
                    _ => Err("expected one byte"),
                }
            }
        }

        impl binary_compat::CompatFingerprint for MultiManualFixture {
            fn compat_fingerprint(&self) -> Vec<u8> {
                vec![self.0]
            }
        }

        #[derive(binary_compat::CompatSampler, binary_compat::CompatFingerprint)]
        struct FixtureValue {
            id: u8,
            flag: bool,
        }

        impl binary_compat::CompatSerializer for FixtureValue {
            fn compat_serialize(&self) -> Vec<u8> {
                vec![self.id, u8::from(self.flag)]
            }
        }

        impl binary_compat::CompatDeserializer for FixtureValue {
            type Error = &'static str;

            fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
                match bytes {
                    [id, flag @ 0..=1] => Ok(Self {
                        id: *id,
                        flag: *flag == 1,
                    }),
                    _ => Err("expected id byte and bool byte"),
                }
            }
        }

        #[derive(binary_compat::CompatSampler, binary_compat::CompatFingerprint)]
        struct LegacyVersionedFixtureValue {
            id: u8,
        }

        impl binary_compat::CompatSerializer for LegacyVersionedFixtureValue {
            fn compat_serialize(&self) -> Vec<u8> {
                vec![self.id]
            }
        }

        #[derive(binary_compat::CompatFingerprint)]
        struct CurrentVersionedFixtureValue {
            id: u8,
            #[compat(fingerprint_since = 2)]
            label: Option<String>,
        }

        impl binary_compat::CompatDeserializer for CurrentVersionedFixtureValue {
            type Error = &'static str;

            fn compat_deserialize(bytes: &[u8]) -> Result<Self, Self::Error> {
                match bytes {
                    [id] => Ok(Self {
                        id: *id,
                        label: None,
                    }),
                    _ => Err("expected id byte"),
                }
            }
        }

        fn metadata() -> binary_compat::DeserializeFixtureMetadata<'static> {
            binary_compat::DeserializeFixtureMetadata::new("toy bytes", "binary_compat tests")
        }

        fn temp_path(name: &str) -> std::path::PathBuf {
            std::env::temp_dir().join(format!("binary_compat_{name}_{}.json", std::process::id()))
        }

        fn sha256_hex(bytes: &[u8]) -> String {
            hex::encode(Sha256::digest(bytes))
        }

        #[test]
        fn fixture_metadata_macro_uses_current_crate() {
            let metadata = binary_compat::deserialize_fixture_metadata!("toy bytes");

            assert_eq!(metadata.format, "toy bytes");
            assert_eq!(
                metadata.producer,
                concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
            );

            let explicit =
                binary_compat::deserialize_fixture_metadata!("toy bytes", producer = "custom");
            assert_eq!(explicit.producer, "custom");
        }

        #[test]
        fn fixture_generation_is_deterministic() {
            let first = temp_path("deterministic_first");
            let second = temp_path("deterministic_second");

            binary_compat::write_deserialize_fixture::<FixtureValue>(&first, 4, metadata())
                .unwrap();
            binary_compat::write_deserialize_fixture::<FixtureValue>(&second, 4, metadata())
                .unwrap();

            let first_json = std::fs::read_to_string(&first).unwrap();
            let second_json = std::fs::read_to_string(&second).unwrap();

            assert_eq!(first_json, second_json);
            assert!(first_json.contains("\"payloads\""));
            assert!(first_json.contains("\"semantic_digest\""));
            assert!(first_json.contains("\"fingerprint_version\""));

            std::fs::remove_file(first).ok();
            std::fs::remove_file(second).ok();
        }

        #[test]
        fn fixture_assertion_succeeds_for_unchanged_semantics() {
            let path = temp_path("unchanged");
            binary_compat::write_deserialize_fixture::<FixtureValue>(&path, 5, metadata()).unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();

            binary_compat::assert_deserialize_fixture::<FixtureValue>(&fixture_json).unwrap();

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_uses_fixture_fingerprint_version() {
            let path = temp_path("versioned_fingerprint");
            binary_compat::write_deserialize_fixture::<LegacyVersionedFixtureValue>(
                &path,
                5,
                metadata(),
            )
            .unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();

            binary_compat::assert_deserialize_fixture::<CurrentVersionedFixtureValue>(
                &fixture_json,
            )
            .unwrap();

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_defaults_missing_fingerprint_version_to_v1() {
            let path = temp_path("missing_fingerprint_version");
            binary_compat::write_deserialize_fixture::<LegacyVersionedFixtureValue>(
                &path,
                5,
                metadata(),
            )
            .unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();
            let mut fixture: serde_json::Value = serde_json::from_str(&fixture_json).unwrap();
            fixture
                .as_object_mut()
                .unwrap()
                .remove("fingerprint_version");
            let fixture_json = serde_json::to_string(&fixture).unwrap();

            binary_compat::assert_deserialize_fixture::<CurrentVersionedFixtureValue>(
                &fixture_json,
            )
            .unwrap();

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_detects_payload_corruption() {
            let path = temp_path("payload_corruption");
            binary_compat::write_deserialize_fixture::<FixtureValue>(&path, 1, metadata()).unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();
            let mut fixture: serde_json::Value = serde_json::from_str(&fixture_json).unwrap();
            fixture["payloads"][0] = serde_json::Value::String("ff".to_owned());
            let fixture_json = serde_json::to_string(&fixture).unwrap();

            assert!(matches!(
                binary_compat::assert_deserialize_fixture::<FixtureValue>(&fixture_json),
                Err(binary_compat::FixtureError::PayloadDigestMismatch { .. })
            ));

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_reports_decode_failure_with_index() {
            let path = temp_path("decode_failure");
            binary_compat::write_deserialize_fixture::<FixtureValue>(&path, 1, metadata()).unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();
            let mut fixture: serde_json::Value = serde_json::from_str(&fixture_json).unwrap();
            fixture["payloads"][0] = serde_json::Value::String("ff".to_owned());
            fixture["payload_digest"] = serde_json::Value::String(sha256_hex(&[0xff]));
            let fixture_json = serde_json::to_string(&fixture).unwrap();

            match binary_compat::assert_deserialize_fixture::<FixtureValue>(&fixture_json) {
                Err(binary_compat::FixtureError::Decode { index, error, .. }) => {
                    assert_eq!(index, 0);
                    assert!(error.contains("expected id byte"));
                }
                other => panic!("expected decode error, got {other:?}"),
            }

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_ignores_informational_metadata() {
            let path = temp_path("metadata_only");
            binary_compat::write_deserialize_fixture::<FixtureValue>(&path, 2, metadata()).unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();
            let mut fixture: serde_json::Value = serde_json::from_str(&fixture_json).unwrap();
            fixture["type_name"] = serde_json::Value::String("renamed::Type".to_owned());
            fixture["format"] = serde_json::Value::String("different label".to_owned());
            fixture["producer"] = serde_json::Value::String("other crate 9.9.9".to_owned());
            fixture["seed"] = serde_json::Value::String("custom seed name".to_owned());
            let mutated = serde_json::to_string(&fixture).unwrap();

            binary_compat::assert_deserialize_fixture::<FixtureValue>(&mutated).unwrap();

            std::fs::remove_file(path).ok();
        }

        #[test]
        fn fixture_assertion_detects_semantic_mismatch() {
            let path = temp_path("semantic_mismatch");
            binary_compat::write_deserialize_fixture::<FixtureValue>(&path, 2, metadata()).unwrap();
            let fixture_json = std::fs::read_to_string(&path).unwrap();
            let mut fixture: serde_json::Value = serde_json::from_str(&fixture_json).unwrap();
            fixture["semantic_digest"] = serde_json::Value::String("00".repeat(32).to_owned());
            let fixture_json = serde_json::to_string(&fixture).unwrap();

            assert!(matches!(
                binary_compat::assert_deserialize_fixture::<FixtureValue>(&fixture_json),
                Err(binary_compat::FixtureError::SemanticDigestMismatch { .. })
            ));

            std::fs::remove_file(path).ok();
        }
    }
}
