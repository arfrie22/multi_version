use proc_macro2::Span;
use std::default::Default;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, Ident, LitInt, LitStr, Token, Variant,
};

pub mod kw {
    use syn::custom_keyword;
    pub use syn::token::Crate;

    // variant metadata
    custom_keyword!(implemented);
    custom_keyword!(deprecated);
    custom_keyword!(alternative_version);
}

pub enum VariantMeta {
    Implemented {
        kw: kw::implemented,
        value: LitStr,
    },
    Deprecated {
        kw: kw::deprecated,
        value: LitStr,
    },
    AlternativeVersion {
        kw: kw::alternative_version,
        versions: Vec<(LitStr, LitInt)>,
    },
}

impl Parse for VariantMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::implemented) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::Implemented { kw, value })
        } else if lookahead.peek(kw::deprecated) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::Deprecated { kw, value })
        } else if lookahead.peek(kw::alternative_version) {
            let kw = input.parse()?;
            let content;
            parenthesized!(content in input);
            let versions = content.parse_terminated::<_, Token![,]>(Version::parse)?;
            Ok(VariantMeta::AlternativeVersion {
                kw,
                versions: versions
                    .into_iter()
                    .map(|Version(version, value)| (version, value))
                    .collect(),
            })
        } else {
            Err(lookahead.error())
        }
    }
}

struct Version(LitStr, LitInt);

impl Parse for Version {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let version = input.parse()?;
        let _: Token![,] = input.parse()?;
        let value = input.parse()?;

        Ok(Version(version, value))
    }
}

impl Spanned for VariantMeta {
    fn span(&self) -> Span {
        match self {
            VariantMeta::Implemented { kw, .. } => kw.span,
            VariantMeta::Deprecated { kw, .. } => kw.span,
            VariantMeta::AlternativeVersion { kw, .. } => kw.span,
        }
    }
}

pub trait VariantExt {
    /// Get all the metadata associated with an enum variant.
    fn get_metadata(&self) -> syn::Result<Vec<VariantMeta>>;
}

impl VariantExt for Variant {
    fn get_metadata(&self) -> syn::Result<Vec<VariantMeta>> {
        get_metadata_inner("multi_version", &self.attrs)
    }
}

fn get_metadata_inner<'a, T: Parse + Spanned>(
    ident: &str,
    it: impl IntoIterator<Item = &'a Attribute>,
) -> syn::Result<Vec<T>> {
    it.into_iter()
        .filter(|attr| attr.path.is_ident(ident))
        .try_fold(Vec::new(), |mut vec, attr| {
            vec.extend(attr.parse_args_with(Punctuated::<T, Token![,]>::parse_terminated)?);
            Ok(vec)
        })
}

pub trait HasMultiVersionVariantProperties {
    fn get_variant_properties(&self) -> syn::Result<MultiVersionVariantProperties>;
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct MultiVersionVariantProperties {
    pub implemented: Option<LitStr>,
    pub deprecated: Option<LitStr>,
    pub alternate_versions: Vec<(LitStr, LitInt)>,
    serialize: Vec<LitStr>,
    to_string: Option<LitStr>,
    ident: Option<Ident>,
}

impl MultiVersionVariantProperties {}

impl HasMultiVersionVariantProperties for Variant {
    fn get_variant_properties(&self) -> syn::Result<MultiVersionVariantProperties> {
        let mut output = MultiVersionVariantProperties {
            ident: Some(self.ident.clone()),
            ..Default::default()
        };

        let mut implemented_kw = None;
        let mut deprecated_kw = None;
        for meta in self.get_metadata()? {
            match meta {
                VariantMeta::Implemented { value, kw } => {
                    if let Some(fst_kw) = implemented_kw {
                        return Err(occurrence_error(fst_kw, kw, "implemented"));
                    }

                    implemented_kw = Some(kw);
                    output.implemented = Some(value);
                }
                VariantMeta::Deprecated { value, kw } => {
                    if let Some(fst_kw) = deprecated_kw {
                        return Err(occurrence_error(fst_kw, kw, "deprecated"));
                    }

                    deprecated_kw = Some(kw);
                    output.deprecated = Some(value);
                }
                VariantMeta::AlternativeVersion { versions, .. } => {
                    output.alternate_versions.extend(versions);
                }
            }
        }

        Ok(output)
    }
}

pub fn occurrence_error<T: quote::ToTokens>(fst: T, snd: T, attr: &str) -> syn::Error {
    let mut e = syn::Error::new_spanned(
        snd,
        format!("Found multiple occurrences of multi_version({})", attr),
    );
    e.combine(syn::Error::new_spanned(fst, "first one here"));
    e
}
