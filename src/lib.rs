#![recursion_limit = "128"]

extern crate proc_macro;

mod multi_version;
mod properties;

use syn::DeriveInput;

#[proc_macro_derive(MultiVersion, attributes(multi_version))]
pub fn derive_multi_version(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = multi_version::derive_multi_version_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    toks.into()
}
