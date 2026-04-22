use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(CryptoHash, attributes(literal))]
pub fn crypto_hash_derive(input: TokenStream) -> TokenStream {
    let crate_name =
        match proc_macro_crate::crate_name("vcs-core").expect("could not find crate name") {
            FoundCrate::Itself => format_ident!("crate"),
            FoundCrate::Name(name) => format_ident!("{}", name),
        };
    let mod_name = quote! {
        #crate_name::crypto::digest
    };

    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        Data::Struct(data_struct) => {
            struct_derive(mod_name, input.ident, data_struct, input.generics).into()
        }
        Data::Enum(data_enum) => enum_derive(input.ident, data_enum).into(),
        Data::Union(..) => quote! {
            compile_error!("`CryptoHash` cannot be derived for unions")
        }
        .into(),
    }
}

fn struct_derive(
    mod_name: TokenStream2,
    type_ident: syn::Ident,
    data_struct: syn::DataStruct,
    generics: syn::Generics,
) -> TokenStream2 {
    let field_stmt: Vec<_> = match data_struct.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| {
                let name = field
                    .ident
                    .as_ref()
                    .expect("named fields should have idents");
                wrap_crypto_hash_fn(&mod_name, quote! { self.#name })
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .zip(0..)
            .map(|(_, i)| wrap_crypto_hash_fn(&mod_name, quote! { self.#i }))
            .collect(),
        Fields::Unit => {
            vec![quote! {}]
        }
    };
    let inner_block = quote! {
        #(#field_stmt)*
    };
    wrap_impl(mod_name, type_ident, generics, inner_block)
}

fn enum_derive(type_ident: syn::Ident, data_enum: syn::DataEnum) -> TokenStream2 {
    quote! {
        compile_error("enums are not yet supported!");
    }
}

fn wrap_crypto_hash_fn(mod_name: &TokenStream2, arg: TokenStream2) -> TokenStream2 {
    quote! {
        #mod_name::CryptoHash::crypto_hash(&#arg, state);
    }
}

fn wrap_impl(
    mod_name: TokenStream2,
    type_ident: syn::Ident,
    generics: syn::Generics,
    inner_block: TokenStream2,
) -> TokenStream2 {
    let syn::Generics {
        params: generic_params,
        where_clause,
        ..
    } = &generics;

    let generic_args = generic_args(generic_params.iter());

    quote! {
        impl #generics #mod_name::CryptoHash for #type_ident #generic_args
        #where_clause
        {
            #[inline]
            #[allow(non_camel_case_types)]
            fn crypto_hash<
                __crypto_hash_derive_D:
                    #mod_name::CryptoDigest,
                __crypto_hash_derive_H:
                    #mod_name::CryptoHasher<Output = __crypto_hash_derive_D>,
            >(
                &self,
                state: &mut __crypto_hash_derive_H,
            ) {
                #inner_block
            }
        }
    }
}

fn generic_args<'a>(params: impl Iterator<Item = &'a syn::GenericParam>) -> TokenStream2 {
    let arg: Vec<_> = params
        .map(|param| match param {
            syn::GenericParam::Lifetime(syn::LifetimeParam { lifetime, .. }) => {
                quote! { #lifetime }
            }
            syn::GenericParam::Type(syn::TypeParam { ident, .. }) => {
                quote! { #ident }
            }
            syn::GenericParam::Const(syn::ConstParam { ident, .. }) => {
                quote! { #ident }
            }
        })
        .collect();

    quote! {
        < #(#arg),* >
    }
}
