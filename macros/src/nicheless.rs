use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Data, DeriveInput, Error, Fields, FieldsNamed,
    Generics, Member, Token, Variant,
};

use crate::UNION_ERR;

pub struct Item {
    pub generics: Generics,
    pub ident: Ident,
    pub data: ItemData,
}

pub enum ItemData {
    Struct(Fields),
    Enum(Punctuated<Variant, Token![,]>),
    Union(FieldsNamed),
}

impl Item {
    pub fn to_type_tokens(&self) -> TokenStream {
        let ident = &self.ident;
        let (_, ty_generics, _) = self.generics.split_for_impl();

        quote! {
            #ident #ty_generics
        }
    }
}

impl TryFrom<DeriveInput> for Item {
    type Error = Error;

    fn try_from(input: DeriveInput) -> Result<Self, Self::Error> {
        let data = match input.data {
            Data::Struct(data) => ItemData::Struct(data.fields),
            Data::Enum(data) => ItemData::Enum(data.variants),
            Data::Union(data) => ItemData::Union(data.fields),
        };

        Ok(Item {
            ident: input.ident,
            generics: input.generics,
            data,
        })
    }
}

impl ToTokens for Item {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.ident;
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        match &self.data {
            ItemData::Struct(fields) => {
                let punct = match fields {
                    Fields::Named(_) => quote!(),
                    _ => quote!(;),
                };
                tokens.extend(quote! {
                    struct #ident #impl_generics #where_clause #fields #punct
                })
            }
            ItemData::Enum(variants) => tokens.extend(quote! {
                enum #ident #impl_generics #where_clause { #variants }
            }),
            ItemData::Union(fields) => tokens.extend(quote! {
                union #ident #impl_generics #where_clause #fields
            }),
        }
    }
}

fn fields_are_nicheless(fields: &Fields) -> TokenStream {
    let fields = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            field
                .ident
                .clone()
                .map(Member::Named)
                .unwrap_or_else(|| Member::Unnamed(i.into()))
        })
        .map(|member| {
            quote! {
                nicheless(value.#member);
            }
        });
    quote! {
        #(#fields)*
    }
}

pub fn impl_nicheless(input: &Item) -> Result<TokenStream, Error> {
    let ident = &input.ident;

    let assertion = match &input.data {
        ItemData::Struct(fields) => {
            let fields = fields_are_nicheless(fields);
            quote! {
                const _: () = {
                    fn all_fields_are_nicheless(value: #ident) {
                        fn nicheless(n: impl iffi::Nicheless) {}

                        #fields
                    }
                };
            }
        }
        ItemData::Enum(variants) => {
            let variants = variants.iter().map(|variant| {
                let fields = fields_are_nicheless(&variant.fields);
                let variant = &variant.ident;
                quote! {
                    #ident::#variant => {
                        #fields
                    },
                }
            });

            quote! {
                const _: () = {
                    fn all_fields_are_nicheless(value: #ident) {
                        fn nicheless(n: impl iffi::Nicheless) {}

                        match {
                            #(#variants)*
                        }
                    }
                };
            }
        }
        ItemData::Union(_) => return Err(Error::new(input.span(), UNION_ERR)),
    };

    Ok(quote! {
        #assertion

        unsafe impl iffi::Nicheless for #ident {}
    })
}
