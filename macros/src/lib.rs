use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parenthesized, parse_macro_input, parse_quote,
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
    Data, DeriveInput, Error, Expr, Field, FieldMutability, Fields, FieldsNamed, FieldsUnnamed,
    Index, LitInt, Meta, Token, Type, Visibility,
};

mod nicheless;
use nicheless::{Item, ItemData};

use crate::attrs::{FieldData, Superset};
mod attrs;

#[proc_macro_derive(Nicheless)]
pub fn derive_nicheless(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = derive_input
        .try_into()
        .and_then(|i| nicheless::impl_nicheless(&i))
        .unwrap_or_else(|e| e.to_compile_error());

    result.into()
}

#[proc_macro_derive(Iffi, attributes(iffi))]
pub fn derive_iffi(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = impl_iffi(derive_input).unwrap_or_else(|e| e.to_compile_error());

    result.into()
}

fn fields_def(fields: &Fields) -> Result<Fields, Error> {
    fn field_def(field: &Field) -> Result<Field, Error> {
        let ty = &field.ty;
        let data = FieldData::from_attrs(&field.attrs)?;
        let superset = match data.superset {
            Superset::Default => quote! {
                iffi::MaybeInvalid<#ty>
            },
            Superset::Type(ty) => ty.to_token_stream(),
        };

        Ok(Field {
            colon_token: field.colon_token,
            mutability: FieldMutability::None,
            vis: field.vis.clone(),
            attrs: Vec::new(),
            ident: field.ident.clone(),
            ty: Type::Verbatim(superset),
        })
    }

    match fields {
        Fields::Unit => Ok(Fields::Unit),
        Fields::Named(FieldsNamed { named, brace_token }) => {
            let fields: Punctuated<Field, _> = named
                .pairs()
                .map(|pair| match pair {
                    Pair::End(value) => field_def(value).map(Pair::End),
                    Pair::Punctuated(value, &punct) => {
                        Ok(Pair::Punctuated(field_def(value)?, punct))
                    }
                })
                .collect::<Result<_, _>>()?;
            Ok(Fields::Named(FieldsNamed {
                brace_token: *brace_token,
                named: fields,
            }))
        }
        Fields::Unnamed(FieldsUnnamed {
            unnamed,
            paren_token,
        }) => {
            let fields: Punctuated<Field, _> = unnamed
                .pairs()
                .map(|pair| match pair {
                    Pair::End(value) => field_def(value).map(Pair::End),
                    Pair::Punctuated(value, &punct) => {
                        Ok(Pair::Punctuated(field_def(value)?, punct))
                    }
                })
                .collect::<Result<_, _>>()?;
            Ok(Fields::Unnamed(FieldsUnnamed {
                paren_token: *paren_token,
                unnamed: fields,
            }))
        }
    }
}

fn fields_check<'a>(
    proxy_fields: impl Iterator<Item = &'a Field>,
    real_fields: impl Iterator<Item = &'a Field>,
) -> Result<TokenStream, Error> {
    fn field_check(
        i: usize,
        proxy_field: &Field,
        real_field: &Field,
    ) -> Result<TokenStream, Error> {
        let index = &Index::from(i);
        let access = proxy_field
            .ident
            .as_ref()
            .map_or_else(|| index.to_token_stream(), |ident| ident.to_token_stream());
        let proxy_ty = &proxy_field.ty;
        let real_ty = &real_field.ty;
        Ok(quote! {
            <#real_ty as iffi::Iffi<#proxy_ty>>::can_transmute(&superset.#access)?
        })
    }
    let mut field = Vec::new();
    proxy_fields
        .zip(real_fields)
        .enumerate()
        .map(|(i, (proxy_field, real_field))| field_check(i, proxy_field, real_field))
        .try_for_each(|x| x.map(|x| field.push(x)))?;
    Ok(quote! {
        #( #field; )*

        Ok(())
    })
}

struct Repr {
    layout: LayoutRepr,
    align: Option<usize>,
}
enum LayoutRepr {
    C,
    Transparent,
}

impl ToTokens for Repr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let layout = match &self.layout {
            LayoutRepr::C => quote!(C),
            LayoutRepr::Transparent => quote!(transparent),
        };
        let align = self.align.map(|n| {
            let align = Index::from(n).into_token_stream();
            quote!(align(#align))
        });
        let mut list = Vec::new();
        list.push(layout);
        list.extend(align);

        tokens.extend(quote! {
            #[repr(
                #(#list),*
            )]
        })
    }
}

const MISSING_REPR_ERR: &str = "Expected type to be `#[repr(C)]` or `#[repr(transparent)]`";
const UNION_ERR: &str = "Iffi does not support unions!";

fn sanitize_fields(fields: &Fields) -> Fields {
    match fields {
        Fields::Named(FieldsNamed { named: fields, .. })
        | Fields::Unnamed(FieldsUnnamed {
            unnamed: fields, ..
        }) => Fields::Named(FieldsNamed {
            brace_token: Default::default(),
            named: fields
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let member = field
                        .ident
                        .as_ref()
                        .map(|ident| ident.to_string())
                        .unwrap_or_else(|| i.to_string());
                    Field {
                        colon_token: Some(Default::default()),
                        ident: Some(Ident::new(&format!("field_{member}"), field.span())),
                        attrs: field.attrs.clone(),
                        mutability: field.mutability.clone(),
                        ty: field.ty.clone(),
                        vis: field.vis.clone(),
                    }
                })
                .collect(),
        }),
        Fields::Unit => Fields::Named(FieldsNamed {
            brace_token: Default::default(),
            named: Default::default(),
        }),
    }
}

fn impl_iffi(input: DeriveInput) -> Result<TokenStream, Error> {
    let mut repr = None;
    let mut enum_repr = None;
    let mut repr_align = None;
    let mut attrs = Vec::new();

    input.attrs.iter().try_for_each(|attr| {
        if attr.path().is_ident("repr") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("C") {
                    repr = Some(LayoutRepr::C);
                } else if meta.path.is_ident("transparent") {
                    repr = Some(LayoutRepr::Transparent);
                } else if meta.path.is_ident("align") {
                    let content;
                    parenthesized!(content in meta.input);
                    let lit: LitInt = content.parse()?;
                    repr_align = Some(lit.base10_parse()?);
                } else if meta.path.is_ident("packed") {
                    return Err(Error::new(
                        meta.path.span(),
                        "Iffi does not support `#[repr(packed)]`!",
                    ));
                } else if let Some(ident) = meta.path.get_ident().map(ToString::to_string) {
                    match ident.as_str() {
                        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32"
                        | "i64" | "i128" | "isize" => {
                            let ident = Ident::new(&ident, meta.path.span());
                            repr = Some(LayoutRepr::C);
                            enum_repr = Some(ident);
                        }
                        _ => (),
                    }
                }

                Ok(())
            })?;
        } else if attr.path().is_ident("iffi_attr") {
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;

            attrs.extend(nested);
        }

        Ok::<_, Error>(())
    })?;

    let repr = repr.ok_or_else(|| Error::new(Span::call_site(), MISSING_REPR_ERR))?;

    let repr = Repr {
        layout: repr,
        align: repr_align,
    };

    let ident = &input.ident;

    let check_expr = match &input.data {
        Data::Struct(data) => {
            let fields_def = fields_def(&data.fields)?;
            let fields_check = fields_check(fields_def.iter(), data.fields.iter())?;

            let item_data = ItemData::Struct(fields_def);
            let item = Item {
                generics: input.generics.clone(),
                ident: Ident::new("Fields", data.fields.span()),
                data: item_data,
            };

            let fields_ty = item.to_type_tokens();

            quote! {
                #repr
                #item

                let superset: &#fields_ty = unsafe { ::core::mem::transmute(superset) };

                #fields_check
            }
        }
        Data::Enum(data) => {
            fn variant_fields_ident(ident: &Ident) -> Ident {
                Ident::new(&format!("{}Fields", ident), ident.span())
            }

            let enum_repr = enum_repr.ok_or_else(|| {
                Error::new(
                    Span::call_site(),
                    "enums require a primitive representation (`#[repr(u8, isize, etc.)]`)",
                )
            })?;
            let discriminant_type = Type::Verbatim(enum_repr.into_token_stream());

            let variant_structs = data
                .variants
                .iter()
                .map(|variant| {
                    let named_fields = sanitize_fields(&variant.fields);
                    let mut fields = fields_def(&named_fields)?;
                    match &mut fields {
                        Fields::Named(FieldsNamed { named, .. }) => named.insert(
                            0,
                            Field {
                                attrs: Vec::new(),
                                colon_token: Some(Default::default()),
                                ident: Some(Ident::new("tag", Span::call_site())),
                                mutability: FieldMutability::None,
                                ty: discriminant_type.clone(),
                                vis: Visibility::Inherited,
                            },
                        ),
                        _ => unreachable!(),
                    }

                    Ok(Item {
                        generics: input.generics.clone(),
                        data: ItemData::Struct(fields),
                        ident: variant_fields_ident(&variant.ident),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let union_fields = FieldsNamed {
                brace_token: Default::default(),
                named: data
                    .variants
                    .iter()
                    .map(|variant| {
                        let variant_fields = variant_fields_ident(&variant.ident);
                        Field {
                            attrs: Vec::new(),
                            colon_token: Some(Default::default()),
                            mutability: FieldMutability::None,
                            vis: Visibility::Inherited,
                            ty: Type::Verbatim(quote! {
                                ::core::mem::ManuallyDrop<#variant_fields>
                            }),
                            ident: Some(variant.ident.clone()),
                        }
                    })
                    .collect(),
            };

            let mut match_arms = Vec::new();
            let mut base_discriminant_expr: Expr = parse_quote!(0);
            let mut discriminant_offset = 0;
            for (variant_struct, enum_variant) in variant_structs.iter().zip(&data.variants) {
                let variant_fields = sanitize_fields(&enum_variant.fields);

                if let Some((_, expr)) = &enum_variant.discriminant {
                    base_discriminant_expr = expr.clone();
                    discriminant_offset = 0;
                }

                let ItemData::Struct(variant_struct_fields) = &variant_struct.data else {
                    unreachable!();
                };

                let fields_check =
                    fields_check(variant_struct_fields.iter().skip(1), variant_fields.iter())?;
                let offset = Index::from(discriminant_offset);
                let variant_ty = variant_struct.to_type_tokens();

                let union_field = &enum_variant.ident;

                let arm = quote! {
                    x if x == {
                        const EXPR: #discriminant_type = #base_discriminant_expr + #offset;
                        EXPR
                    } => {

                        // SAFETY: just verified that the discriminant is correct.
                        let superset: &#variant_ty = unsafe { &superset.#union_field };

                        #fields_check
                    }
                };
                match_arms.push(arm);
                discriminant_offset += 1;
            }

            let item = Item {
                generics: input.generics.clone(),
                data: ItemData::Union(union_fields),
                ident: Ident::new("Variants", data.variants.span()),
            };

            quote! {
                #(
                    #[repr(C)]
                    #variant_structs
                )*

                #repr
                #[allow(non_snake_case)]
                #item

                let tag = unsafe {::core::ptr::read(superset as *const _ as *const #discriminant_type) };
                let superset: &Variants = unsafe { ::core::mem::transmute(superset) };

                match tag {
                    #(
                        #match_arms,
                    )*
                    v => Err(iffi::Error::new::<Self, iffi::MaybeInvalid<Self>>(iffi::ErrorKind::InvalidEnumDiscriminant(iffi::BitPattern::from_le(&v.to_le_bytes())))),
                }
            }
        }
        Data::Union(_) => return Err(Error::new(input.span(), UNION_ERR)),
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        // SAFETY: universe has same layout and check_expr is valid.
        unsafe impl #impl_generics iffi::Iffi for #ident #ty_generics #where_clause {
            fn can_transmute(superset: &iffi::MaybeInvalid<Self>) -> Result<(), iffi::Error> {
                #check_expr
            }
        }
    })
}
