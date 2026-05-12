use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span};
use quote::{format_ident, quote};
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, ExprLit, Fields, Index, Item, ItemEnum, ItemStruct,
    Lit, LitInt, LitStr, MetaNameValue, Path, Result, Token, Type, WherePredicate,
    parse_macro_input, parse_quote,
};

#[proc_macro_attribute]
pub fn compat_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as CompatArgs);
    let parsed_item = parse_macro_input!(item as Item);

    match expand_compat_test(args, parsed_item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn compat_deserialize_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as DeserializeCompatArgs);
    let parsed_item = parse_macro_input!(item as Item);

    match expand_compat_deserialize_test(args, parsed_item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(CompatSampler, attributes(compat))]
pub fn derive_compat_sampler(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_compat_sampler(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(CompatFingerprint, attributes(compat))]
pub fn derive_compat_fingerprint(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_compat_fingerprint(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(CompatShape, attributes(compat))]
pub fn derive_compat_shape(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_compat_shape(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(BincodeSerializer, attributes(compat))]
pub fn derive_bincode_serializer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_bincode_serializer(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(BincodeDeserializer, attributes(compat))]
pub fn derive_bincode_deserializer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_bincode_deserializer(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(WincodeSerializer, attributes(compat))]
pub fn derive_wincode_serializer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_wincode_serializer(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(WincodeDeserializer, attributes(compat))]
pub fn derive_wincode_deserializer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_derive_wincode_deserializer(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

struct CompatArgs {
    digest: LitStr,
    samples: Option<LitInt>,
    shape_digest: Option<LitStr>,
}

impl Parse for CompatArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut digest = None;
        let mut samples = None;
        let mut shape_digest = None;

        while !input.is_empty() {
            let name_value: MetaNameValue = input.parse()?;
            let Some(ident) = name_value.path.get_ident() else {
                return Err(Error::new_spanned(
                    name_value.path,
                    "expected `digest = ...`, `samples = ...`, or `shape_digest = ...`",
                ));
            };

            match ident.to_string().as_str() {
                "digest" => {
                    if digest.is_some() {
                        return Err(Error::new_spanned(ident, "duplicate `digest` argument"));
                    }
                    digest = Some(parse_lit_str(name_value.value, "digest")?);
                }
                "samples" => {
                    if samples.is_some() {
                        return Err(Error::new_spanned(ident, "duplicate `samples` argument"));
                    }
                    samples = Some(parse_lit_int(name_value.value, "samples")?);
                }
                "shape_digest" => {
                    if shape_digest.is_some() {
                        return Err(Error::new_spanned(
                            ident,
                            "duplicate `shape_digest` argument",
                        ));
                    }
                    shape_digest = Some(parse_lit_str(name_value.value, "shape_digest")?);
                }
                _ => {
                    return Err(Error::new_spanned(
                        ident,
                        "unknown argument; expected `digest`, `samples`, or `shape_digest`",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }

        let Some(digest) = digest else {
            return Err(Error::new(
                Span::call_site(),
                "missing required `digest` argument",
            ));
        };

        Ok(Self {
            digest,
            samples,
            shape_digest,
        })
    }
}

struct DeserializeCompatArgs {
    fixtures: Vec<NamedFixture>,
}

struct NamedFixture {
    name: Ident,
    fixture: LitStr,
}

impl Parse for DeserializeCompatArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut fixtures = Vec::new();
        let mut saw_fixture = false;
        let mut saw_fixtures = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;

            match ident.to_string().as_str() {
                "fixture" => {
                    if saw_fixture {
                        return Err(Error::new_spanned(ident, "duplicate `fixture` argument"));
                    }
                    if saw_fixtures {
                        return Err(Error::new_spanned(
                            ident,
                            "use either `fixture = ...` or `fixtures(...)`, not both",
                        ));
                    }
                    saw_fixture = true;
                    input.parse::<Token![=]>()?;
                    let fixture = parse_lit_str_expr(input.parse()?, "fixture")?;
                    validate_fixture_path(&fixture, "fixture")?;
                    fixtures.push(NamedFixture {
                        name: format_ident!("deserialization_fixture"),
                        fixture,
                    });
                }
                "fixtures" => {
                    if saw_fixtures {
                        return Err(Error::new_spanned(ident, "duplicate `fixtures` argument"));
                    }
                    if saw_fixture {
                        return Err(Error::new_spanned(
                            ident,
                            "use either `fixture = ...` or `fixtures(...)`, not both",
                        ));
                    }
                    saw_fixtures = true;

                    let content;
                    parenthesized!(content in input);
                    while !content.is_empty() {
                        let name: Ident = content.parse()?;
                        if fixtures.iter().any(|fixture| fixture.name == name) {
                            return Err(Error::new_spanned(name, "duplicate fixture name"));
                        }
                        content.parse::<Token![=]>()?;
                        let fixture = parse_lit_str_expr(content.parse()?, "fixture")?;
                        validate_fixture_path(&fixture, "fixture")?;
                        fixtures.push(NamedFixture { name, fixture });

                        if content.is_empty() {
                            break;
                        }

                        content.parse::<Token![,]>()?;
                        if content.is_empty() {
                            break;
                        }
                    }

                    if fixtures.is_empty() {
                        return Err(Error::new_spanned(
                            ident,
                            "`fixtures(...)` must contain at least one named fixture",
                        ));
                    }
                }
                _ => {
                    return Err(Error::new_spanned(
                        ident,
                        "unknown argument; expected `fixture` or `fixtures`",
                    ));
                }
            };

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }

        if fixtures.is_empty() {
            return Err(Error::new(
                Span::call_site(),
                "missing required `fixture` or `fixtures(...)` argument",
            ));
        }

        Ok(Self { fixtures })
    }
}

fn parse_lit_str(expr: Expr, name: &str) -> Result<LitStr> {
    parse_lit_str_expr(expr, name)
}

fn parse_lit_str_expr(expr: Expr, name: &str) -> Result<LitStr> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Ok(value),
        other => Err(Error::new_spanned(
            other,
            format!("`{name}` must be a string literal"),
        )),
    }
}

fn validate_fixture_path(fixture: &LitStr, name: &str) -> Result<()> {
    if fixture.value().is_empty() {
        return Err(Error::new_spanned(
            fixture,
            format!("`{name}` path must not be empty"),
        ));
    }

    Ok(())
}

fn parse_lit_int(expr: Expr, name: &str) -> Result<LitInt> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => Ok(value),
        other => Err(Error::new_spanned(
            other,
            format!("`{name}` must be an integer literal"),
        )),
    }
}

fn expand_compat_test(args: CompatArgs, item: Item) -> Result<proc_macro2::TokenStream> {
    enum TargetItem {
        Struct(ItemStruct),
        Enum(ItemEnum),
    }

    let target = match item {
        Item::Struct(item_struct) => TargetItem::Struct(item_struct),
        Item::Enum(item_enum) => TargetItem::Enum(item_enum),
        Item::Union(item_union) => {
            return Err(Error::new_spanned(
                item_union.union_token,
                "`compat_test` currently supports concrete structs and enums only, not unions",
            ));
        }
        other => {
            return Err(Error::new_spanned(
                other,
                "`compat_test` can only be applied to a struct or enum",
            ));
        }
    };

    let (ident, generics) = match &target {
        TargetItem::Struct(item_struct) => (&item_struct.ident, &item_struct.generics),
        TargetItem::Enum(item_enum) => (&item_enum.ident, &item_enum.generics),
    };

    if !generics.params.is_empty() {
        return Err(Error::new_spanned(
            generics,
            "`compat_test` currently supports concrete, non-generic structs and enums only",
        ));
    }

    let expected = decode_digest(&args.digest)?;
    let expected_shape = args.shape_digest.as_ref().map(decode_digest).transpose()?;
    let samples = match args.samples {
        Some(samples) => {
            let value = samples.base10_parse::<usize>()?;
            if value == 0 {
                return Err(Error::new_spanned(
                    samples,
                    "`samples` must be greater than zero",
                ));
            }
            quote!(#value)
        }
        None => {
            let runtime_crate = runtime_crate_path()?;
            quote!(#runtime_crate::DEFAULT_SAMPLES)
        }
    };

    let runtime_crate = runtime_crate_path()?;
    let ident = ident.clone();
    let test_module = format_ident!("__binary_compat_{}", ident);
    let type_name = ident.to_string();
    let item_tokens = match target {
        TargetItem::Struct(item_struct) => quote!(#item_struct),
        TargetItem::Enum(item_enum) => quote!(#item_enum),
    };

    let expected_bytes = expected.iter().copied();
    let shape_test = if let Some(expected_shape) = expected_shape {
        let expected_shape_bytes = expected_shape.iter().copied();
        quote! {
            #[test]
            fn shape_digest() {
                const EXPECTED: [u8; 32] = [#(#expected_shape_bytes),*];

                let actual = #runtime_crate::compat_shape_digest::<super::#ident>();

                if let Err(message) =
                    #runtime_crate::check_shape_digest(#type_name, EXPECTED, actual)
                {
                    panic!("{message}");
                }
            }
        }
    } else {
        quote!()
    };

    Ok(quote! {
        #item_tokens

        #[cfg(test)]
        #[allow(non_snake_case)]
        mod #test_module {
            #[test]
            fn serialization_digest() {
                const EXPECTED: [u8; 32] = [#(#expected_bytes),*];
                const SAMPLES: usize = #samples;

                let actual = #runtime_crate::compat_digest::<super::#ident>(SAMPLES);

                if let Err(message) =
                    #runtime_crate::check_digest(#type_name, EXPECTED, actual, SAMPLES)
                {
                    panic!("{message}");
                }
            }

            #shape_test
        }
    })
}

fn expand_compat_deserialize_test(
    args: DeserializeCompatArgs,
    item: Item,
) -> Result<proc_macro2::TokenStream> {
    enum TargetItem {
        Struct(ItemStruct),
        Enum(ItemEnum),
    }

    let target = match item {
        Item::Struct(item_struct) => TargetItem::Struct(item_struct),
        Item::Enum(item_enum) => TargetItem::Enum(item_enum),
        Item::Union(item_union) => {
            return Err(Error::new_spanned(
                item_union.union_token,
                "`compat_deserialize_test` currently supports concrete structs and enums only, not unions",
            ));
        }
        other => {
            return Err(Error::new_spanned(
                other,
                "`compat_deserialize_test` can only be applied to a struct or enum",
            ));
        }
    };

    let (ident, generics) = match &target {
        TargetItem::Struct(item_struct) => (&item_struct.ident, &item_struct.generics),
        TargetItem::Enum(item_enum) => (&item_enum.ident, &item_enum.generics),
    };

    if !generics.params.is_empty() {
        return Err(Error::new_spanned(
            generics,
            "`compat_deserialize_test` currently supports concrete, non-generic structs and enums only",
        ));
    }

    let ident = ident.clone();
    let runtime_crate = runtime_crate_path()?;
    let fixture_tests = args.fixtures.iter().map(|fixture| {
        let name = &fixture.name;
        let path = &fixture.fixture;

        quote! {
            #[test]
            fn #name() {
                if let Err(error) = #runtime_crate::assert_deserialize_fixture::<super::#ident>(
                    include_str!(#path),
                ) {
                    panic!("{error}");
                }
            }
        }
    });
    let test_module = format_ident!("__binary_compat_deserialize_{}", ident);
    let item_tokens = match target {
        TargetItem::Struct(item_struct) => quote!(#item_struct),
        TargetItem::Enum(item_enum) => quote!(#item_enum),
    };

    Ok(quote! {
        #item_tokens

        #[cfg(test)]
        #[allow(non_snake_case)]
        mod #test_module {
            #(#fixture_tests)*
        }
    })
}

fn decode_digest(digest: &LitStr) -> Result<[u8; 32]> {
    let value = digest.value();
    if value.len() != 64 {
        return Err(Error::new_spanned(
            digest,
            "`digest` must be exactly 64 lowercase hexadecimal characters",
        ));
    }

    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new_spanned(
            digest,
            "`digest` must contain only lowercase hexadecimal characters",
        ));
    }

    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&value[start..start + 2], 16).map_err(|_| {
            Error::new_spanned(digest, "`digest` must be valid lowercase hexadecimal")
        })?;
    }

    Ok(out)
}

fn runtime_crate_path() -> Result<proc_macro2::TokenStream> {
    match crate_name("binary_compat") {
        Ok(FoundCrate::Itself) => Ok(quote!(crate)),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            Ok(quote!(::#ident))
        }
        Err(error) => Err(Error::new(
            Span::call_site(),
            format!("could not find `binary_compat` dependency: {error}"),
        )),
    }
}

enum FieldStrategy {
    Trait,
    SampleWith(Path),
    Default,
    Value(Expr),
}

fn expand_derive_compat_sampler(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;

    let body = match input.data {
        Data::Struct(data) => derive_struct_body(&runtime_crate, &mut generics, data.fields)?,
        Data::Enum(data) => derive_enum_body(&runtime_crate, &mut generics, data)?,
        Data::Union(data) => {
            return Err(Error::new_spanned(
                data.union_token,
                "`derive(CompatSampler)` currently supports structs and enums only, not unions",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatSampler for #name #ty_generics #where_clause {
            fn compat_sample<R>(rng: &mut R) -> Self
            where
                R: #runtime_crate::RngCore + ?Sized,
            {
                #body
            }
        }
    })
}

fn derive_runtime_crate_path(attrs: &[Attribute]) -> Result<proc_macro2::TokenStream> {
    let mut override_path = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("compat")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                if override_path.is_some() {
                    return Err(meta.error("duplicate `crate` override"));
                }
                let value = meta.value()?;
                override_path = Some(value.parse::<Path>()?);
                Ok(())
            } else if meta.path.is_ident("bincode") {
                let value = meta.value()?;
                let _ = value.parse::<LitStr>()?;
                Ok(())
            } else {
                Err(meta.error(
                    "unknown container attribute; expected `crate = ...` or `bincode = ...`",
                ))
            }
        })?;
    }

    match override_path {
        Some(path) => Ok(quote!(#path)),
        None => runtime_crate_path(),
    }
}

#[derive(Clone, Copy)]
enum BincodeVersion {
    Auto,
    One,
    Two,
}

fn derive_bincode_version(attrs: &[Attribute]) -> Result<BincodeVersion> {
    let mut version = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("compat")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }

            if meta.path.is_ident("bincode") {
                if version.is_some() {
                    return Err(meta.error("duplicate `bincode` override"));
                }
                let value = meta.value()?.parse::<LitStr>()?;
                version = Some(match value.value().as_str() {
                    "1" | "bincode1" => BincodeVersion::One,
                    "2" | "bincode2" => BincodeVersion::Two,
                    other => {
                        return Err(meta.error(format!(
                            "unsupported `bincode` value `{other}`; expected \"1\" or \"2\""
                        )));
                    }
                });
                return Ok(());
            }

            Err(meta
                .error("unknown container attribute; expected `crate = ...` or `bincode = ...`"))
        })?;
    }

    Ok(version.unwrap_or(BincodeVersion::Auto))
}

fn derive_struct_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    fields: Fields,
) -> Result<proc_macro2::TokenStream> {
    match fields {
        Fields::Named(fields) => {
            let values = fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().expect("named field has an ident");
                    let expr = field_sample_expr(runtime_crate, generics, &field.attrs, &field.ty)?;
                    Ok(quote!(#name: #expr))
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(quote!(Self { #(#values),* }))
        }
        Fields::Unnamed(fields) => {
            let values = fields
                .unnamed
                .iter()
                .map(|field| field_sample_expr(runtime_crate, generics, &field.attrs, &field.ty))
                .collect::<Result<Vec<_>>>()?;

            Ok(quote!(Self(#(#values),*)))
        }
        Fields::Unit => Ok(quote!(Self)),
    }
}

fn derive_enum_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    data: syn::DataEnum,
) -> Result<proc_macro2::TokenStream> {
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            data.enum_token,
            "`derive(CompatSampler)` cannot sample an enum with no variants",
        ));
    }

    let variant_count = data.variants.len();
    let arms = data
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let variant_name = &variant.ident;
            let body = derive_variant_body(runtime_crate, generics, variant_name, &variant.fields)?;
            Ok(quote!(#index => #body))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        match (#runtime_crate::RngCore::next_u64(rng) as usize) % #variant_count {
            #(#arms,)*
            _ => unreachable!("binary_compat enum variant index is always in range"),
        }
    })
}

fn derive_variant_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    variant_name: &Ident,
    fields: &Fields,
) -> Result<proc_macro2::TokenStream> {
    match fields {
        Fields::Named(fields) => {
            let values = fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().expect("named field has an ident");
                    let expr = field_sample_expr(runtime_crate, generics, &field.attrs, &field.ty)?;
                    Ok(quote!(#name: #expr))
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(quote!(Self::#variant_name { #(#values),* }))
        }
        Fields::Unnamed(fields) => {
            let values = fields
                .unnamed
                .iter()
                .map(|field| field_sample_expr(runtime_crate, generics, &field.attrs, &field.ty))
                .collect::<Result<Vec<_>>>()?;

            Ok(quote!(Self::#variant_name(#(#values),*)))
        }
        Fields::Unit => Ok(quote!(Self::#variant_name)),
    }
}

fn field_sample_expr(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    attrs: &[Attribute],
    ty: &Type,
) -> Result<proc_macro2::TokenStream> {
    match parse_field_strategy(attrs)? {
        FieldStrategy::Trait => {
            push_where_predicate(generics, parse_quote!(#ty: #runtime_crate::CompatSampler));
            Ok(quote!(<#ty as #runtime_crate::CompatSampler>::compat_sample(rng)))
        }
        FieldStrategy::SampleWith(path) => Ok(quote!(#path(rng))),
        FieldStrategy::Default => {
            push_where_predicate(generics, parse_quote!(#ty: ::core::default::Default));
            Ok(quote!(<#ty as ::core::default::Default>::default()))
        }
        FieldStrategy::Value(expr) => Ok(quote!(#expr)),
    }
}

fn push_where_predicate(generics: &mut syn::Generics, predicate: WherePredicate) {
    generics.make_where_clause().predicates.push(predicate);
}

fn parse_field_strategy(attrs: &[Attribute]) -> Result<FieldStrategy> {
    let mut strategy = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("compat")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("fingerprint_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("fingerprint_skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`fingerprint_skip` does not take a value"));
                }
                return Ok(());
            }
            if meta.path.is_ident("fingerprint_since") {
                let value = meta.value()?;
                let _ = value.parse::<LitInt>()?;
                return Ok(());
            }
            if meta.path.is_ident("shape_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("shape_skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`shape_skip` does not take a value"));
                }
                return Ok(());
            }

            let next = if meta.path.is_ident("sample_with") {
                let value = meta.value()?;
                FieldStrategy::SampleWith(value.parse::<Path>()?)
            } else if meta.path.is_ident("value") {
                let value = meta.value()?;
                FieldStrategy::Value(value.parse::<Expr>()?)
            } else if meta.path.is_ident("default") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`default` does not take a value"));
                }
                FieldStrategy::Default
            } else if meta.path.is_ident("skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`skip` does not take a value"));
                }
                FieldStrategy::Default
            } else {
                return Err(meta.error(
                    "unknown field attribute; expected `sample_with`, `value`, `default`, `skip`, `fingerprint_with`, `fingerprint_skip`, `fingerprint_since`, `shape_with`, or `shape_skip`",
                ));
            };

            if strategy.is_some() {
                return Err(meta.error("only one sampling strategy is allowed per field"));
            }
            strategy = Some(next);
            Ok(())
        })?;
    }

    Ok(strategy.unwrap_or(FieldStrategy::Trait))
}

enum FingerprintStrategy {
    Trait { since: u32 },
    With { path: Path, since: u32 },
    Skip,
}

struct FingerprintBody {
    body: proc_macro2::TokenStream,
    version: proc_macro2::TokenStream,
}

fn expand_derive_compat_fingerprint(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;

    let fingerprint = match input.data {
        Data::Struct(data) => fingerprint_struct_body(&runtime_crate, &mut generics, data.fields)?,
        Data::Enum(data) => fingerprint_enum_body(&runtime_crate, &mut generics, data)?,
        Data::Union(data) => {
            return Err(Error::new_spanned(
                data.union_token,
                "`derive(CompatFingerprint)` currently supports structs and enums only, not unions",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let fingerprint_version = &fingerprint.version;
    let fingerprint_body = &fingerprint.body;

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatFingerprint for #name #ty_generics #where_clause {
            const COMPAT_FINGERPRINT_VERSION: u32 = #fingerprint_version;

            fn compat_fingerprint(&self) -> ::std::vec::Vec<u8> {
                self.compat_fingerprint_with(
                    #runtime_crate::FingerprintContext::latest::<Self>(),
                )
            }

            fn compat_fingerprint_with(
                &self,
                __binary_compat_context: #runtime_crate::FingerprintContext,
            ) -> ::std::vec::Vec<u8> {
                #fingerprint_body
            }
        }
    })
}

fn fingerprint_struct_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    fields: Fields,
) -> Result<FingerprintBody> {
    let (statements, versions) = match fields {
        Fields::Named(fields) => {
            let fields = fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().expect("named field has an ident");
                    fingerprint_field(
                        runtime_crate,
                        generics,
                        &field.attrs,
                        &field.ty,
                        quote!(&self.#name),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            split_fingerprint_fields(fields)
        }
        Fields::Unnamed(fields) => {
            let fields = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let index = Index::from(index);
                    fingerprint_field(
                        runtime_crate,
                        generics,
                        &field.attrs,
                        &field.ty,
                        quote!(&self.#index),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            split_fingerprint_fields(fields)
        }
        Fields::Unit => (Vec::new(), Vec::new()),
    };

    let version = max_version_expr(versions);

    Ok(FingerprintBody {
        version,
        body: quote! {
            let mut __binary_compat_out = ::std::vec::Vec::new();
            #(#statements)*
            __binary_compat_out
        },
    })
}

fn fingerprint_enum_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    data: syn::DataEnum,
) -> Result<FingerprintBody> {
    if data.variants.is_empty() {
        return Ok(FingerprintBody {
            version: quote!(1),
            body: quote!(match *self {}),
        });
    }
    let mut variant_versions = Vec::new();

    let arms = data
        .variants
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            let variant_name = &variant.ident;
            let variant_index = variant_index as u32;

            let (pattern, statements) = match &variant.fields {
                Fields::Named(fields) => {
                    let mut pattern_fields = Vec::new();
                    let mut fingerprint_fields = Vec::new();

                    for (index, field) in fields.named.iter().enumerate() {
                        let field_name = field.ident.as_ref().expect("named field has an ident");
                        let binding = format_ident!("__binary_compat_field_{index}");
                        pattern_fields.push(quote!(#field_name: #binding));
                        fingerprint_fields.push(fingerprint_field(
                            runtime_crate,
                            generics,
                            &field.attrs,
                            &field.ty,
                            quote!(#binding),
                        )?);
                    }
                    let (statements, versions) = split_fingerprint_fields(fingerprint_fields);
                    variant_versions.push(max_version_expr(versions));

                    (
                        quote!(Self::#variant_name { #(#pattern_fields),* }),
                        statements,
                    )
                }
                Fields::Unnamed(fields) => {
                    let mut bindings = Vec::new();
                    let mut fingerprint_fields = Vec::new();

                    for (index, field) in fields.unnamed.iter().enumerate() {
                        let binding = format_ident!("__binary_compat_field_{index}");
                        bindings.push(binding.clone());
                        fingerprint_fields.push(fingerprint_field(
                            runtime_crate,
                            generics,
                            &field.attrs,
                            &field.ty,
                            quote!(#binding),
                        )?);
                    }
                    let (statements, versions) = split_fingerprint_fields(fingerprint_fields);
                    variant_versions.push(max_version_expr(versions));

                    (quote!(Self::#variant_name(#(#bindings),*)), statements)
                }
                Fields::Unit => {
                    variant_versions.push(quote!(1));
                    (quote!(Self::#variant_name), Vec::new())
                }
            };

            Ok(quote! {
                #pattern => {
                    let mut __binary_compat_out = ::std::vec::Vec::new();
                    __binary_compat_out.extend_from_slice(&(#variant_index as u32).to_le_bytes());
                    #(#statements)*
                    __binary_compat_out
                }
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let version = max_version_expr(variant_versions);

    Ok(FingerprintBody {
        version,
        body: quote! {
            match self {
                #(#arms,)*
            }
        },
    })
}

struct FingerprintField {
    statement: proc_macro2::TokenStream,
    version: proc_macro2::TokenStream,
}

fn split_fingerprint_fields(
    fields: Vec<FingerprintField>,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    fields
        .into_iter()
        .map(|field| (field.statement, field.version))
        .unzip()
}

fn max_version_expr(versions: Vec<proc_macro2::TokenStream>) -> proc_macro2::TokenStream {
    versions.into_iter().fold(quote!(1), |left, right| {
        quote! {
            if #left > #right { #left } else { #right }
        }
    })
}

fn fingerprint_field(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    attrs: &[Attribute],
    ty: &Type,
    accessor: proc_macro2::TokenStream,
) -> Result<FingerprintField> {
    match parse_fingerprint_strategy(attrs)? {
        FingerprintStrategy::Trait { since } => {
            push_where_predicate(
                generics,
                parse_quote!(#ty: #runtime_crate::CompatFingerprint),
            );
            let version = quote! {
                if #since > <#ty as #runtime_crate::CompatFingerprint>::COMPAT_FINGERPRINT_VERSION {
                    #since
                } else {
                    <#ty as #runtime_crate::CompatFingerprint>::COMPAT_FINGERPRINT_VERSION
                }
            };
            Ok(FingerprintField {
                version,
                statement: quote! {
                    if __binary_compat_context.version() >= #since {
                    let __binary_compat_part =
                        <#ty as #runtime_crate::CompatFingerprint>::compat_fingerprint_with(
                            #accessor,
                            __binary_compat_context,
                        );
                    #runtime_crate::append_fingerprint_part(
                        &mut __binary_compat_out,
                        &__binary_compat_part,
                    );
                    }
                },
            })
        }
        FingerprintStrategy::With { path, since } => Ok(FingerprintField {
            version: quote!(#since),
            statement: quote! {
                if __binary_compat_context.version() >= #since {
                let __binary_compat_part = #path(#accessor);
                #runtime_crate::append_fingerprint_part(
                    &mut __binary_compat_out,
                    &__binary_compat_part,
                );
                }
            },
        }),
        FingerprintStrategy::Skip => Ok(FingerprintField {
            version: quote!(1),
            statement: quote!(),
        }),
    }
}

fn parse_fingerprint_strategy(attrs: &[Attribute]) -> Result<FingerprintStrategy> {
    let mut strategy = None;
    let mut since = 1_u32;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("compat")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("sample_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("value") {
                let value = meta.value()?;
                let _ = value.parse::<Expr>()?;
                return Ok(());
            }
            if meta.path.is_ident("default") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`default` does not take a value"));
                }
                return Ok(());
            }
            if meta.path.is_ident("skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`skip` does not take a value"));
                }
                return Ok(());
            }
            if meta.path.is_ident("shape_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("shape_skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`shape_skip` does not take a value"));
                }
                return Ok(());
            }
            if meta.path.is_ident("fingerprint_since") {
                let value = meta.value()?;
                since = value.parse::<LitInt>()?.base10_parse::<u32>()?;
                if since == 0 {
                    return Err(meta.error("`fingerprint_since` must be greater than zero"));
                }
                return Ok(());
            }

            let next = if meta.path.is_ident("fingerprint_with") {
                let value = meta.value()?;
                FingerprintStrategy::With {
                    path: value.parse::<Path>()?,
                    since,
                }
            } else if meta.path.is_ident("fingerprint_skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`fingerprint_skip` does not take a value"));
                }
                FingerprintStrategy::Skip
            } else {
                return Err(meta.error(
                    "unknown field attribute; expected `sample_with`, `value`, `default`, `skip`, `fingerprint_with`, `fingerprint_skip`, `fingerprint_since`, `shape_with`, or `shape_skip`",
                ));
            };

            if strategy.is_some() {
                return Err(meta.error("only one fingerprint strategy is allowed per field"));
            }
            strategy = Some(next);
            Ok(())
        })?;
    }

    Ok(match strategy {
        Some(FingerprintStrategy::Trait { .. }) => FingerprintStrategy::Trait { since },
        Some(FingerprintStrategy::With { path, .. }) => FingerprintStrategy::With { path, since },
        Some(FingerprintStrategy::Skip) => FingerprintStrategy::Skip,
        None => FingerprintStrategy::Trait { since },
    })
}

enum ShapeStrategy {
    Trait,
    With(Path),
    Skip,
}

fn expand_derive_compat_shape(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;
    let type_name = name.to_string();

    let body = match input.data {
        Data::Struct(data) => {
            shape_struct_body(&runtime_crate, &mut generics, &type_name, data.fields)?
        }
        Data::Enum(data) => shape_enum_body(&runtime_crate, &mut generics, &type_name, data)?,
        Data::Union(data) => {
            return Err(Error::new_spanned(
                data.union_token,
                "`derive(CompatShape)` currently supports structs and enums only, not unions",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatShape for #name #ty_generics #where_clause {
            fn compat_shape() -> ::std::vec::Vec<u8> {
                #body
            }
        }
    })
}

fn shape_struct_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    type_name: &str,
    fields: Fields,
) -> Result<proc_macro2::TokenStream> {
    let (kind, statements) = match fields {
        Fields::Named(fields) => {
            let statements = fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().expect("named field has an ident");
                    shape_field_statement(
                        runtime_crate,
                        generics,
                        &field.attrs,
                        &field.ty,
                        Some(name.to_string()),
                        None,
                    )
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            ("struct", statements)
        }
        Fields::Unnamed(fields) => {
            let statements = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    shape_field_statement(
                        runtime_crate,
                        generics,
                        &field.attrs,
                        &field.ty,
                        None,
                        Some(index),
                    )
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            ("tuple_struct", statements)
        }
        Fields::Unit => ("unit_struct", Vec::new()),
    };
    let field_count = statements.len() as u64;

    Ok(quote! {
        let mut __binary_compat_shape = ::std::vec::Vec::new();
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, #kind.as_bytes());
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, #type_name.as_bytes());
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, &#field_count.to_le_bytes());
        #(#statements)*
        __binary_compat_shape
    })
}

fn shape_enum_body(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    type_name: &str,
    data: syn::DataEnum,
) -> Result<proc_macro2::TokenStream> {
    let variant_count = data.variants.len() as u64;
    let statements = data
        .variants
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            let variant_name = variant.ident.to_string();
            let variant_index = variant_index as u64;
            let (kind, field_statements) = match &variant.fields {
                Fields::Named(fields) => {
                    let statements = fields
                        .named
                        .iter()
                        .map(|field| {
                            let name = field.ident.as_ref().expect("named field has an ident");
                            shape_field_statement(
                                runtime_crate,
                                generics,
                                &field.attrs,
                                &field.ty,
                                Some(name.to_string()),
                                None,
                            )
                        })
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    ("struct_variant", statements)
                }
                Fields::Unnamed(fields) => {
                    let statements = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            shape_field_statement(
                                runtime_crate,
                                generics,
                                &field.attrs,
                                &field.ty,
                                None,
                                Some(index),
                            )
                        })
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    ("tuple_variant", statements)
                }
                Fields::Unit => ("unit_variant", Vec::new()),
            };
            let field_count = field_statements.len() as u64;

            Ok(quote! {
                #runtime_crate::append_shape_part(&mut __binary_compat_shape, b"variant");
                #runtime_crate::append_shape_part(&mut __binary_compat_shape, &#variant_index.to_le_bytes());
                #runtime_crate::append_shape_part(&mut __binary_compat_shape, #variant_name.as_bytes());
                #runtime_crate::append_shape_part(&mut __binary_compat_shape, #kind.as_bytes());
                #runtime_crate::append_shape_part(&mut __binary_compat_shape, &#field_count.to_le_bytes());
                #(#field_statements)*
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        let mut __binary_compat_shape = ::std::vec::Vec::new();
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, b"enum");
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, #type_name.as_bytes());
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, &#variant_count.to_le_bytes());
        #(#statements)*
        __binary_compat_shape
    })
}

fn shape_field_statement(
    runtime_crate: &proc_macro2::TokenStream,
    generics: &mut syn::Generics,
    attrs: &[Attribute],
    ty: &Type,
    name: Option<String>,
    index: Option<usize>,
) -> Result<Option<proc_macro2::TokenStream>> {
    let shape_expr = match parse_shape_strategy(attrs)? {
        ShapeStrategy::Trait => {
            push_where_predicate(generics, parse_quote!(#ty: #runtime_crate::CompatShape));
            quote!(<#ty as #runtime_crate::CompatShape>::compat_shape())
        }
        ShapeStrategy::With(path) => quote!(#path()),
        ShapeStrategy::Skip => return Ok(None),
    };

    let label = match name {
        Some(name) => quote!(#name.as_bytes()),
        None => {
            let index = index.expect("tuple field has index") as u64;
            quote!(&#index.to_le_bytes())
        }
    };

    Ok(Some(quote! {
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, b"field");
        #runtime_crate::append_shape_part(&mut __binary_compat_shape, #label);
        let __binary_compat_field_shape = #shape_expr;
        #runtime_crate::append_shape_part(
            &mut __binary_compat_shape,
            &__binary_compat_field_shape,
        );
    }))
}

fn parse_shape_strategy(attrs: &[Attribute]) -> Result<ShapeStrategy> {
    let mut strategy = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("compat")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("sample_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("value") {
                let value = meta.value()?;
                let _ = value.parse::<Expr>()?;
                return Ok(());
            }
            if meta.path.is_ident("default")
                || meta.path.is_ident("skip")
                || meta.path.is_ident("fingerprint_skip")
            {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("this attribute does not take a value"));
                }
                return Ok(());
            }
            if meta.path.is_ident("fingerprint_with") {
                let value = meta.value()?;
                let _ = value.parse::<Path>()?;
                return Ok(());
            }
            if meta.path.is_ident("fingerprint_since") {
                let value = meta.value()?;
                let _ = value.parse::<LitInt>()?;
                return Ok(());
            }

            let next = if meta.path.is_ident("shape_with") {
                let value = meta.value()?;
                ShapeStrategy::With(value.parse::<Path>()?)
            } else if meta.path.is_ident("shape_skip") {
                if meta.input.peek(Token![=]) {
                    return Err(meta.error("`shape_skip` does not take a value"));
                }
                ShapeStrategy::Skip
            } else {
                return Err(meta.error(
                    "unknown field attribute; expected `sample_with`, `value`, `default`, `skip`, `fingerprint_with`, `fingerprint_skip`, `fingerprint_since`, `shape_with`, or `shape_skip`",
                ));
            };

            if strategy.is_some() {
                return Err(meta.error("only one shape strategy is allowed per field"));
            }
            strategy = Some(next);
            Ok(())
        })?;
    }

    Ok(strategy.unwrap_or(ShapeStrategy::Trait))
}

fn expand_derive_bincode_serializer(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let bincode_version = derive_bincode_version(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;
    let self_ty = self_type(&name, &generics);
    let serialize_trait = match bincode_version {
        BincodeVersion::Auto => {
            quote!(#runtime_crate::__private::BincodeAutoCompatSerializeRequiresOneBincodeFeatureOrBincodeAttribute)
        }
        BincodeVersion::One => quote!(#runtime_crate::__private::Bincode1CompatSerialize),
        BincodeVersion::Two => quote!(#runtime_crate::__private::Bincode2CompatSerialize),
    };

    push_where_predicate(&mut generics, parse_quote!(#self_ty: #serialize_trait));

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatSerializer for #name #ty_generics #where_clause {
            fn compat_serialize(&self) -> ::std::vec::Vec<u8> {
                <#self_ty as #serialize_trait>::bincode_compat_serialize(self)
            }
        }
    })
}

fn expand_derive_bincode_deserializer(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let bincode_version = derive_bincode_version(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;
    let self_ty = self_type(&name, &generics);
    let deserialize_trait = match bincode_version {
        BincodeVersion::Auto => {
            quote!(#runtime_crate::__private::BincodeAutoCompatDeserializeRequiresOneBincodeFeatureOrBincodeAttribute)
        }
        BincodeVersion::One => quote!(#runtime_crate::__private::Bincode1CompatDeserialize),
        BincodeVersion::Two => quote!(#runtime_crate::__private::Bincode2CompatDeserialize),
    };

    push_where_predicate(&mut generics, parse_quote!(#self_ty: #deserialize_trait));

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatDeserializer for #name #ty_generics #where_clause {
            type Error = <#self_ty as #deserialize_trait>::Error;

            fn compat_deserialize(bytes: &[u8]) -> ::core::result::Result<Self, Self::Error> {
                <#self_ty as #deserialize_trait>::bincode_compat_deserialize(bytes)
            }
        }
    })
}

fn expand_derive_wincode_serializer(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;
    let self_ty = self_type(&name, &generics);

    push_where_predicate(
        &mut generics,
        parse_quote!(#self_ty: #runtime_crate::__private::wincode::SchemaWrite<#runtime_crate::__private::wincode::config::DefaultConfig, Src = #self_ty>),
    );

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatSerializer for #name #ty_generics #where_clause {
            fn compat_serialize(&self) -> ::std::vec::Vec<u8> {
                #runtime_crate::__private::wincode::serialize(self)
                    .expect("binary_compat wincode serialization failed")
            }
        }
    })
}

fn expand_derive_wincode_deserializer(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let runtime_crate = derive_runtime_crate_path(&input.attrs)?;
    let name = input.ident;
    let mut generics = input.generics;
    let self_ty = self_type(&name, &generics);

    push_where_predicate(
        &mut generics,
        parse_quote!(for<'__binary_compat_de> #self_ty: #runtime_crate::__private::wincode::SchemaRead<'__binary_compat_de, #runtime_crate::__private::wincode::config::DefaultConfig, Dst = #self_ty>),
    );

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #runtime_crate::CompatDeserializer for #name #ty_generics #where_clause {
            type Error = #runtime_crate::__private::wincode::ReadError;

            fn compat_deserialize(bytes: &[u8]) -> ::core::result::Result<Self, Self::Error> {
                #runtime_crate::__private::wincode::deserialize_exact(bytes)
            }
        }
    })
}

fn self_type(name: &Ident, generics: &syn::Generics) -> Type {
    let (_, ty_generics, _) = generics.split_for_impl();
    parse_quote!(#name #ty_generics)
}
