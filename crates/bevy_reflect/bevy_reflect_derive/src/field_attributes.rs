use crate::REFLECT_ATTRIBUTE_NAME;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Attribute, Meta, NestedMeta};

pub(crate) static IGNORE: &str = "ignore";

/// A container for reflection field configuration.
#[derive(Default)]
pub struct ReflectFieldAttr {
    /// Determines if this field should be ignored.
    pub ignore: bool,
}

/// Parse all field attributes marked "reflect" (such as `#[reflect(ignore)]`).
pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<ReflectFieldAttr, syn::Error> {
    let mut args = ReflectFieldAttr::default();
    let mut errors: Option<syn::Error> = None;

    let attrs = attrs
        .iter()
        .filter(|a| a.path.is_ident(REFLECT_ATTRIBUTE_NAME));
    for attr in attrs {
        let meta = attr.parse_meta()?;
        if let Err(err) = parse_meta(&mut args, &meta) {
            if let Some(ref mut error) = errors {
                error.combine(err);
            } else {
                errors = Some(err);
            }
        }
    }

    if let Some(error) = errors {
        Err(error)
    } else {
        Ok(args)
    }
}

fn parse_meta(args: &mut ReflectFieldAttr, meta: &Meta) -> Result<(), syn::Error> {
    match meta {
        Meta::Path(path) if path.is_ident(IGNORE) => {
            args.ignore = true;
            Ok(())
        }
        Meta::Path(path) => Err(syn::Error::new(
            path.span(),
            format!("unknown attribute parameter: {}", path.to_token_stream()),
        )),
        Meta::NameValue(pair) => {
            let path = &pair.path;
            Err(syn::Error::new(
                path.span(),
                format!("unknown attribute parameter: {}", path.to_token_stream()),
            ))
        }
        Meta::List(list) if !list.path.is_ident(REFLECT_ATTRIBUTE_NAME) => {
            Err(syn::Error::new(list.path.span(), "unexpected property"))
        }
        Meta::List(list) => {
            for nested in list.nested.iter() {
                if let NestedMeta::Meta(meta) = nested {
                    parse_meta(args, meta)?;
                }
            }
            Ok(())
        }
    }
}
