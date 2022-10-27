use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
    Data, DeriveInput, Error, Expr, Field, Fields, FieldsNamed, FieldsUnnamed, Index, Meta,
    NestedMeta, Variant, Visibility,
};

#[proc_macro_attribute]
pub fn iffi(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let original_input: TokenStream = input.clone().into();
    let attr = parse_macro_input!(attr as TopLevelAttr);
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = do_derive_iffi(attr, derive_input).unwrap_or_else(|e| e.to_compile_error());

    quote! {
        #result
        #original_input
    }
    .into()
}

struct TopLevelAttr {
    vis: Visibility,
    ident: Ident,
}

macro_rules! bail {
    ($span:expr, $str:expr) => {
        return Err(Error::new($span, $str))
    };
}

impl Parse for TopLevelAttr {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        Ok(Self {
            vis: input.parse()?,
            ident: input.parse()?,
        })
    }
}

fn fields_def(fields: &Fields) -> Result<TokenStream, Error> {
    fn field_def(field: &Field) -> Result<TokenStream, Error> {
        let ty = &field.ty;
        let vis = &field.vis;
        let ident = field.ident.iter();
        Ok(quote! {
            #vis #( #ident: )* <#ty as iffi::Iffi>::Subset
        })
    }

    match fields {
        Fields::Unit => Ok(quote!(;)),
        Fields::Named(FieldsNamed {
            named,
            brace_token: _,
        }) => {
            let mut fields = Vec::new();
            named
                .iter()
                .map(field_def)
                .try_for_each(|x| x.map(|x| fields.push(x)))?;
            Ok(quote! {
                {
                    #( #fields, )*
                }
            })
        }
        Fields::Unnamed(FieldsUnnamed {
            unnamed,
            paren_token: _,
        }) => {
            let mut fields = Vec::new();
            unnamed
                .iter()
                .map(field_def)
                .try_for_each(|x| x.map(|x| fields.push(x)))?;
            Ok(quote! {
                (
                    #( #fields, )*
                );
            })
        }
    }
}

fn enum_match_arms_check(variant: &Variant) -> Result<TokenStream, Error> {
    let ident = &variant.ident;
    let fields = fields_check(&variant.fields)?;
    Ok(quote! {
        Self::#ident => #fields
    })
}

fn fields_check(fields: &Fields) -> Result<TokenStream, Error> {
    fn field_check((i, field): (usize, &Field)) -> Result<TokenStream, Error> {
        let index = &Index::from(i);
        let access = field
            .ident
            .as_ref()
            .map_or_else(|| index.to_token_stream(), |ident| ident.to_token_stream());
        Ok(quote! {
            iffi::Iffi::can_transmute(&self.#access)?
        })
    }

    match fields {
        Fields::Unit => Ok(quote!()),
        Fields::Named(FieldsNamed {
            named: fields,
            brace_token: _,
        })
        | Fields::Unnamed(FieldsUnnamed {
            unnamed: fields,
            paren_token: _,
        }) => {
            let mut field = Vec::new();
            fields
                .iter()
                .enumerate()
                .map(field_check)
                .try_for_each(|x| x.map(|x| field.push(x)))?;
            Ok(quote! {
                #( #field; )*
            })
        }
    }
}

fn do_derive_iffi(
    TopLevelAttr {
        vis,
        ident: notsafe,
    }: TopLevelAttr,
    input: DeriveInput,
) -> Result<TokenStream, Error> {
    // if safe.is_none() {
    //     return Err(Error::new(
    //         Span::call_site(),
    //         "Expected an `iffi` attribute with the name of the safe version (e.g. `#[iffi(pub MyUnsafeType)]`)",
    //     ));
    // }
    // let TopLevelAttr {
    //     vis,
    //     ident: notsafe,
    // } = safe.unwrap();

    const MISSING_REPR_ERR: &str = "Expected type to be `#[repr(C)]`, `#[repr(C, align(x))]`";

    let mut repr = None;
    let mut enum_repr = None;

    for attr in input.attrs.iter().filter_map(|attr| attr.parse_meta().ok()) {
        match attr {
            Meta::List(ref list) if list.path.is_ident("repr") => {
                for repr_meta in list.nested.iter() {
                    match repr_meta {
                        nested @ NestedMeta::Meta(Meta::List(ref list))
                            if list.path.is_ident("align") =>
                        {
                            let old = repr.iter();
                            repr = Some(quote!(#( #old, )* #nested));
                        }
                        nested @ NestedMeta::Meta(Meta::Path(ref path)) => {
                            if path.is_ident("C") {
                                let old = repr.iter();
                                repr = Some(quote!(#( #old, )* #nested));
                            } else if let Some(ident) = path.get_ident().map(|i| i.to_string()) {
                                match ident.as_str() {
                                    "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16"
                                    | "i32" | "i64" | "i128" => {
                                        let ident = Ident::new(&ident, repr_meta.span());
                                        let old = repr.iter();
                                        repr = Some(quote!(#( #old, )* C));
                                        enum_repr = Some(ident);
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => bail!(repr_meta.span(), MISSING_REPR_ERR),
                    }
                }
                //list.nested.iter().all(|repr| {
                //matches!(repr, NestedMeta::Meta(Meta::Path(path)) if path.segments.first().map(|seg| seg.ident))
                //|| matches!(repr, NestedMeta::Meta(Meta::Path(path)) if path.is_ident("C"))
                //|| matches!(repr, NestedMeta::Meta(Meta::List(list)) if list.path.is_ident("align"))
                //}) => {
            }
            _ if attr.path().is_ident("repr") => bail!(attr.span(), MISSING_REPR_ERR),
            _ => continue,
        }
    }

    let repr = repr.ok_or_else(|| Error::new(Span::call_site(), MISSING_REPR_ERR))?;

    let input_span = input.span();

    let (item, expr) = match input.data {
        Data::Struct(data) => {
            let def = fields_def(&data.fields)?;
            let check = fields_check(&data.fields)?;
            (
                quote! {
                    #vis struct #notsafe #def
                },
                quote! {
                    #check
                    Ok(())
                },
            )
        }
        Data::Enum(data) => {
            if data
                .variants
                .iter()
                .any(|variant| variant.fields.is_empty())
            {
                let enum_repr = enum_repr.unwrap_or_else(|| Ident::new("u32", Span::call_site()));
                let item = quote! {
                    #vis struct #notsafe (#enum_repr);
                };

                let mut match_arms = Vec::new();
                let mut i: Expr = parse_quote!(0);
                for variant in data.variants.into_iter() {
                    if let Some((_, expr)) = variant.discriminant {
                        i = expr;
                    }
                    let arm = quote! {
                        x if x == {
                            const EXPR: #enum_repr = #i;
                            EXPR
                        }=> Ok(())
                    };
                    match_arms.push(arm);
                    i = parse_quote!(#i + 1);
                }
                let expr = quote! {
                    match self.0 {
                        #(
                            #match_arms,
                        )*
                        v => Err(iffi::Error::InvalidEnumVariant(v as u128)),
                    }
                };

                (item, expr)
            } else {
                (quote!(), quote!())
            }
        }
        Data::Union(_) => bail!(input_span, "Iffi does not support unions!"),
    };

    let ident = input.ident;
    Ok(quote! {
        #[repr(#repr)]
        #item

        // SAFETY: Unsafe guaranteed to have the same layout as Self
        unsafe impl iffi::Iffi for #notsafe {
            type Subset = #ident;

            fn can_transmute(&self) -> Result<(), iffi::Error> {
                #expr
            }
        }
    })
}
