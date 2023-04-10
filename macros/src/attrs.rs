use syn::{parse_str, punctuated::Pair, spanned::Spanned, Attribute, Error, LitStr, Type};

#[derive(Default)]
pub enum Superset {
    #[default]
    Default,
    Type(Type),
}

#[derive(Default)]
pub struct FieldData {
    pub superset: Superset,
}

impl FieldData {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self, Error> {
        let mut data = FieldData::default();

        attrs.iter().try_for_each(|attr| {
            if attr.path().is_ident("iffi") {
                attr.parse_nested_meta(|iffi| {
                    if iffi.path.is_ident("with") {
                        let lit: LitStr = iffi.value()?.parse()?;
                        let ty: Type = parse_str(&lit.value())?;

                        if let Superset::Default = data.superset {
                            data.superset = Superset::Type(ty);
                        } else {
                            return Err(Error::new(
                                lit.span(),
                                "conflicting `#[iffi(with = \"...\")]` attributes",
                            ));
                        }

                        Ok(())
                    } else {
                        Err(Error::new(
                            iffi.path.span(),
                            format_args!(
                                "unknown attribute parameter {}",
                                iffi.path
                                    .segments
                                    .pairs()
                                    .map(|pair| match pair {
                                        Pair::End(segment) => segment.ident.to_string(),
                                        Pair::Punctuated(segment, _) =>
                                            format!("{}::", segment.ident),
                                    })
                                    .collect::<String>(),
                            ),
                        ))
                    }
                })?;
            }

            Ok::<_, Error>(())
        })?;

        Ok(data)
    }
}
