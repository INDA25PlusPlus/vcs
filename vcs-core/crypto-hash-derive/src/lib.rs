use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(CryptoHash, attributes(literal))]
pub fn crypto_hash_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        Data::Struct(data_struct) => { struct_derive(input.ident, data_struct) }.into(),
        Data::Enum(data_enum) => { enum_derive(input.ident, data_enum) }.into(),
        Data::Union(..) => quote! {
            compile_error!("`CryptoHash` cannot be derived for unions")
        }
        .into(),
    }
}

fn struct_derive(type_ident: syn::Ident, data_struct: syn::DataStruct) -> TokenStream2 {
    todo!()
}

fn enum_derive(type_ident: syn::Ident, data_enum: syn::DataEnum) -> TokenStream2 {
    todo!()
}
