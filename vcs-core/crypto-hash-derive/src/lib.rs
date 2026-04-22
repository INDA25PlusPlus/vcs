use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

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

    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    match input.data {
        syn::Data::Struct(data_struct) => {
            struct_derive(mod_name, input.ident, data_struct, input.generics).into()
        }
        syn::Data::Enum(data_enum) => {
            enum_derive(mod_name, input.ident, data_enum, input.generics).into()
        }
        syn::Data::Union(..) => quote! {
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
        syn::Fields::Named(fields) => fields
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
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .zip(0..)
            .map(|(_, i)| wrap_crypto_hash_fn(&mod_name, quote! { self.#i }))
            .collect(),
        syn::Fields::Unit => {
            vec![quote! {}]
        }
    };
    let inner_block = quote! {
        #(#field_stmt)*
    };
    wrap_impl(mod_name, type_ident, generics, inner_block)
}

fn enum_derive(
    mod_name: TokenStream2,
    type_ident: syn::Ident,
    data_enum: syn::DataEnum,
    generics: syn::Generics,
) -> TokenStream2 {
    let variant_stmt: Vec<_> = data_enum
        .variants
        .iter()
        .zip(0u64..)
        .map(|(variant, id)| enum_variant_derive(&mod_name, &type_ident, variant, id))
        .collect();
    let inner_block = quote! {
        match self {
            #(#variant_stmt,)*
        }
    };
    wrap_impl(mod_name, type_ident, generics, inner_block)
}

fn enum_variant_derive(
    mod_name: &TokenStream2,
    type_ident: &syn::Ident,
    variant: &syn::Variant,
    variant_id: u64,
) -> TokenStream2 {
    let variant_ident = &variant.ident;
    match &variant.fields {
        syn::Fields::Named(fields) => {
            let field_name = fields.named.iter().map(|field| {
                field
                    .ident
                    .as_ref()
                    .expect("named fields should have idents")
            });
            let wrapped = field_name
                .clone()
                .map(|ident| wrap_crypto_hash_fn(mod_name, quote! { #ident }));

            quote! {
                #type_ident::#variant_ident { #(#field_name,)* } => {
                    #mod_name::CryptoHasher::write_u64(state, #variant_id);
                    #(#wrapped)*
                }
            }
        }
        syn::Fields::Unnamed(fields) => {
            let field_name = fields
                .unnamed
                .iter()
                .zip(0u64..)
                .map(|(_, field_id)| format_ident!("field_{}", field_id));
            let wrapped = field_name
                .clone()
                .map(|ident| wrap_crypto_hash_fn(mod_name, quote! { #ident }));

            quote! {
                #type_ident::#variant_ident(#(#field_name,)*) => {
                    #mod_name::CryptoHasher::write_u64(state, #variant_id);
                    #(#wrapped)*
                }
            }
        }
        syn::Fields::Unit => {
            quote! {
                #type_ident::#variant_ident => {
                    #mod_name::CryptoHasher::write_u64(state, #variant_id);
                }
            }
        }
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
