use super::properties::HasMultiVersionVariantProperties;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{Data, DeriveInput, PathArguments, Type, TypeParen};

pub fn derive_multi_version_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let attrs = &ast.attrs;

    let mut discriminant_type: Type = syn::parse("usize".parse().unwrap()).unwrap();

    for attr in attrs {
        let path = &attr.path;
        let tokens = &attr.tokens;
        if path.leading_colon.is_some() {
            continue;
        }
        if path.segments.len() != 1 {
            continue;
        }
        let segment = path.segments.first().unwrap();
        if segment.ident != "repr" {
            continue;
        }
        if segment.arguments != PathArguments::None {
            continue;
        }
        let typ_paren = match syn::parse2::<Type>(tokens.clone()) {
            Ok(Type::Paren(TypeParen { elem, .. })) => *elem,
            _ => continue,
        };
        let inner_path = match &typ_paren {
            Type::Path(t) => t,
            _ => continue,
        };
        if let Some(seg) = inner_path.path.segments.last() {
            for t in &[
                "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize",
            ] {
                if seg.ident == t {
                    discriminant_type = typ_paren;
                    break;
                }
            }
        }
    }

    if gen.lifetimes().count() > 0 {
        return Err(syn::Error::new(
            Span::call_site(),
            "This macro doesn't support enums with lifetimes. \
             The resulting enums would be unbounded.",
        ));
    }

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "This macro only supports enums.",
            ))
        }
    };

    let mut implemented_arms = Vec::new();
    let mut deprecated_arms = Vec::new();
    let mut value_arms = Vec::new();
    let mut variant_idents = Vec::new();
    for variant in variants {
        let variant_properties = variant.get_variant_properties()?;

        variant_idents.push(variant.ident.clone());
        let variant_ident = variant.ident.clone();

        if let Some(implemented) = variant_properties.implemented {
            implemented_arms.push(
                quote! { #name::#variant_ident => semver::Version::from_str("#value").unwrap() }
                    .to_string()
                    .replace("#value", &implemented.value())
                    .parse()
                    .unwrap(),
            );
        }

        if let Some(deprecated) = variant_properties.deprecated {
            deprecated_arms.push(quote! { #name::#variant_ident => Some(semver::Version::from_str("#value").unwrap()) }.to_string().replace("#value", &deprecated.value()).parse().unwrap());
        }

        if !variant_properties.alternate_versions.is_empty() {
            let mut match_value = "".to_owned();

            for version in variant_properties.alternate_versions {
                match_value.push_str(&format!("if semver::VersionReq::from_str(\"{}\").unwrap().matches(version) {{{}::from({})}} else ", version.0.value(), discriminant_type.to_token_stream(), version.1.to_token_stream()));
            }

            match_value.push_str(&format!(
                "{{{}::from(*self)}}",
                discriminant_type.to_token_stream()
            ));

            value_arms.push(
                format!("{}::{} => {}", name, variant_ident, match_value)
                    .parse()
                    .unwrap(),
            );
        }
    }

    implemented_arms.push(quote! { _ => semver::Version::new(0, 0, 0) });
    deprecated_arms.push(quote! { _ => None });
    value_arms.push(quote! { _ => #discriminant_type::from(*self) });

    let all_variants = quote! { [
        #(#name::#variant_idents),*
    ] };

    Ok(quote! {
        impl #name {
            #[inline]
            fn implemented_since (&self) -> semver::Version
            {
                match self {
                    #(#implemented_arms),*
                }
            }

            #[inline]
            fn deprecated_since (&self) -> Option<semver::Version>
            {
                match self {
                    #(#deprecated_arms),*
                }
            }

            #[inline]
            fn value_for_version (&self, version: &semver::Version) -> Option<#discriminant_type>
            {
                if self.exists_in(version) {
                Some (match self {
                    #(#value_arms),*
                })
                } else {
                    None
                }
            }

            #[inline]
            fn exists_in (&self, version: &semver::Version) -> bool
            {
                *version >= self.implemented_since() && {
                    if let Some(depricated) = self.deprecated_since() {
                        *version < depricated
                    } else {
                        true
                    }
                }
            }

            #[inline]
            fn get_all_values (version: &semver::Version, skip: Option<&[Self]>) -> Vec<Self>
            {
                let all_variants = #all_variants;
                let mut values = Vec::new();
                let mut skip = skip.unwrap_or(&[]);

                for variant in all_variants.iter() {
                    if !skip.contains(variant) && variant.exists_in(version) {
                        values.push(*variant);
                    }
                }

                values
            }
        }
    })
}
