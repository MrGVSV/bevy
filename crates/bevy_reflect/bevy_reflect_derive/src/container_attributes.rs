//! Contains code related to container attributes for reflected types.
//!
//! A container attribute is an attribute which applies to an entire struct or enum
//! as opposed to a particular field or variant. An example of such an attribute is
//! the derive helper attribute for `Reflect`, which looks like:
//! `#[reflect(PartialEq, Default, ...)]` and `#[reflect_value(PartialEq, Default, ...)]`.

use crate::fq_std::{FQAny, FQOption};
use crate::utility;
use proc_macro2::{Ident, Span};
use quote::quote_spanned;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::{Attribute, LitBool, Path};

// The "special" trait idents that are used internally for reflection.
// Received via attributes like `#[reflect(PartialEq, Hash, ...)]`
const DEBUG_ATTR: &str = "Debug";
const PARTIAL_EQ_ATTR: &str = "PartialEq";
const HASH_ATTR: &str = "Hash";

// The traits listed below are not considered "special" (i.e. they use the `ReflectMyTrait` syntax)
// but useful to know exist nonetheless
pub(crate) const REFLECT_DEFAULT: &str = "ReflectDefault";

// Attributes for `FromReflect` implementation
const FROM_REFLECT_ATTR: &str = "from_reflect";

// The error message to show when a trait/type is specified multiple times
const CONFLICTING_TYPE_DATA_MESSAGE: &str = "conflicting type data registration";

/// A marker for trait implementations registered via the `Reflect` derive macro.
#[derive(Clone, Default)]
pub(crate) enum TraitImpl {
    /// The trait is not registered as implemented.
    #[default]
    NotImplemented,

    /// The trait is registered as implemented.
    Implemented(Span),

    /// The trait is registered with a custom function rather than an actual implementation.
    Custom(Path),
}

impl TraitImpl {
    /// Merges this [`TraitImpl`] with another.
    ///
    /// Update `self` with whichever value is not [`TraitImpl::NotImplemented`].
    /// If `other` is [`TraitImpl::NotImplemented`], then `self` is not modified.
    /// An error is returned if neither value is [`TraitImpl::NotImplemented`].
    pub fn merge(&mut self, other: TraitImpl) -> Result<(), syn::Error> {
        match (&self, other) {
            (TraitImpl::NotImplemented, value) => {
                *self = value;
                Ok(())
            }
            (_, TraitImpl::NotImplemented) => Ok(()),
            (_, TraitImpl::Implemented(span)) => {
                Err(syn::Error::new(span, CONFLICTING_TYPE_DATA_MESSAGE))
            }
            (_, TraitImpl::Custom(path)) => {
                Err(syn::Error::new_spanned(path, CONFLICTING_TYPE_DATA_MESSAGE))
            }
        }
    }
}

/// A collection of attributes used for deriving `FromReflect`.
#[derive(Clone, Default)]
pub(crate) struct FromReflectAttrs {
    auto_derive: Option<LitBool>,
}

impl FromReflectAttrs {
    /// Returns true if `FromReflect` should be automatically derived as part of the `Reflect` derive.
    pub fn should_auto_derive(&self) -> bool {
        self.auto_derive
            .as_ref()
            .map(|lit| lit.value())
            .unwrap_or(true)
    }

    /// Merges this [`FromReflectAttrs`] with another.
    pub fn merge(&mut self, other: FromReflectAttrs) -> Result<(), syn::Error> {
        if let Some(new) = other.auto_derive {
            if let Some(existing) = &self.auto_derive {
                if existing.value() != new.value() {
                    return Err(syn::Error::new(
                        new.span(),
                        format!("`from_reflect` already set to {}", existing.value()),
                    ));
                }
            } else {
                self.auto_derive = Some(new);
            }
        }

        Ok(())
    }
}

/// A collection of traits that have been registered for a reflected type.
///
/// This keeps track of a few traits that are utilized internally for reflection
/// (we'll call these traits _special traits_ within this context), but it
/// will also keep track of all registered traits. Traits are registered as part of the
/// `Reflect` derive macro using the helper attribute: `#[reflect(...)]`.
///
/// The list of special traits are as follows:
/// * `Debug`
/// * `Hash`
/// * `PartialEq`
///
/// When registering a trait, there are a few things to keep in mind:
/// * Traits must have a valid `Reflect{}` struct in scope. For example, `Default`
///   needs `bevy_reflect::prelude::ReflectDefault` in scope.
/// * Traits must be single path identifiers. This means you _must_ use `Default`
///   instead of `std::default::Default` (otherwise it will try to register `Reflectstd`!)
/// * A custom function may be supplied in place of an actual implementation
///   for the special traits (but still follows the same single-path identifier
///   rules as normal).
///
/// # Example
///
/// Registering the `Default` implementation:
///
/// ```ignore
/// // Import ReflectDefault so it's accessible by the derive macro
/// use bevy_reflect::prelude::ReflectDefault.
///
/// #[derive(Reflect, Default)]
/// #[reflect(Default)]
/// struct Foo;
/// ```
///
/// Registering the `Hash` implementation:
///
/// ```ignore
/// // `Hash` is a "special trait" and does not need (nor have) a ReflectHash struct
///
/// #[derive(Reflect, Hash)]
/// #[reflect(Hash)]
/// struct Foo;
/// ```
///
/// Registering the `Hash` implementation using a custom function:
///
/// ```ignore
/// // This function acts as our `Hash` implementation and
/// // corresponds to the `Reflect::reflect_hash` method.
/// fn get_hash(foo: &Foo) -> Option<u64> {
///   Some(123)
/// }
///
/// #[derive(Reflect)]
/// // Register the custom `Hash` function
/// #[reflect(Hash(get_hash))]
/// struct Foo;
/// ```
///
/// > __Note:__ Registering a custom function only works for special traits.
///
#[derive(Default, Clone)]
pub(crate) struct ReflectTraits {
    debug: TraitImpl,
    hash: TraitImpl,
    partial_eq: TraitImpl,
    from_reflect: FromReflectAttrs,
    idents: Vec<Ident>,
}

impl ReflectTraits {
    pub fn with_attr(
        &mut self,
        attr: &Attribute,
        is_from_reflect_derive: bool,
    ) -> Result<(), syn::Error> {
        attr.parse_nested_meta(|meta| self.with_nested_meta(meta, is_from_reflect_derive))
    }

    pub fn with_nested_meta(
        &mut self,
        meta: ParseNestedMeta,
        is_from_reflect_derive: bool,
    ) -> Result<(), syn::Error> {
        if meta.path.is_ident(HASH_ATTR) {
            if meta.input.peek(Paren) {
                meta.parse_nested_meta(|meta| self.hash.merge(TraitImpl::Custom(meta.path)))
            } else {
                self.hash.merge(TraitImpl::Implemented(meta.path.span()))
            }
        } else if meta.path.is_ident(PARTIAL_EQ_ATTR) {
            if meta.input.peek(Paren) {
                meta.parse_nested_meta(|meta| self.partial_eq.merge(TraitImpl::Custom(meta.path)))
            } else {
                self.partial_eq
                    .merge(TraitImpl::Implemented(meta.path.span()))
            }
        } else if meta.path.is_ident(DEBUG_ATTR) {
            if meta.input.peek(Paren) {
                meta.parse_nested_meta(|meta| self.debug.merge(TraitImpl::Custom(meta.path)))
            } else {
                self.debug.merge(TraitImpl::Implemented(meta.path.span()))
            }
        } else if meta.path.is_ident(FROM_REFLECT_ATTR) {
            let from_reflect = FromReflectAttrs {
                auto_derive: if is_from_reflect_derive {
                    Some(LitBool::new(true, Span::call_site()))
                } else {
                    Some(meta.value()?.parse()?)
                },
            };

            self.from_reflect.merge(from_reflect)
        } else {
            // We only track reflected idents for traits not considered special.
            if meta.path.segments.len() != 1 {
                return Err(meta.error("expected single identifier"));
            }
            let ident = &meta.path.segments.last().unwrap().ident;
            let ident_name = ident.to_string();

            // Create the reflect ident
            // We set the span to the old ident so any compile errors point to that ident instead
            let mut reflect_ident = utility::get_reflect_ident(&ident_name);
            reflect_ident.set_span(ident.span());

            add_unique_ident(&mut self.idents, reflect_ident)?;
            Ok(())
        }
    }

    /// Returns true if the given reflected trait name (i.e. `ReflectDefault` for `Default`)
    /// is registered for this type.
    pub fn contains(&self, name: &str) -> bool {
        self.idents.iter().any(|ident| ident == name)
    }

    /// The list of reflected traits by their reflected ident (i.e. `ReflectDefault` for `Default`).
    pub fn idents(&self) -> &[Ident] {
        &self.idents
    }

    /// The `FromReflect` attributes on this type.
    #[allow(clippy::wrong_self_convention)]
    pub fn from_reflect(&self) -> &FromReflectAttrs {
        &self.from_reflect
    }

    /// Returns the implementation of `Reflect::reflect_hash` as a `TokenStream`.
    ///
    /// If `Hash` was not registered, returns `None`.
    pub fn get_hash_impl(&self, bevy_reflect_path: &Path) -> Option<proc_macro2::TokenStream> {
        match &self.hash {
            &TraitImpl::Implemented(span) => Some(quote_spanned! {span=>
                fn reflect_hash(&self) -> #FQOption<u64> {
                    use ::core::hash::{Hash, Hasher};
                    let mut hasher = #bevy_reflect_path::utility::reflect_hasher();
                    Hash::hash(&#FQAny::type_id(self), &mut hasher);
                    Hash::hash(self, &mut hasher);
                    #FQOption::Some(Hasher::finish(&hasher))
                }
            }),
            TraitImpl::Custom(ref impl_fn) => {
                let span = impl_fn.span();
                Some(quote_spanned! {span=>
                    fn reflect_hash(&self) -> #FQOption<u64> {
                        #FQOption::Some(#impl_fn(self))
                    }
                })
            }
            TraitImpl::NotImplemented => None,
        }
    }

    /// Returns the implementation of `Reflect::reflect_partial_eq` as a `TokenStream`.
    ///
    /// If `PartialEq` was not registered, returns `None`.
    pub fn get_partial_eq_impl(
        &self,
        bevy_reflect_path: &Path,
    ) -> Option<proc_macro2::TokenStream> {
        match &self.partial_eq {
            &TraitImpl::Implemented(span) => Some(quote_spanned! {span=>
                fn reflect_partial_eq(&self, value: &dyn #bevy_reflect_path::Reflect) -> #FQOption<bool> {
                    let value = <dyn #bevy_reflect_path::Reflect>::as_any(value);
                    if let #FQOption::Some(value) = <dyn #FQAny>::downcast_ref::<Self>(value) {
                        #FQOption::Some(::core::cmp::PartialEq::eq(self, value))
                    } else {
                        #FQOption::Some(false)
                    }
                }
            }),
            TraitImpl::Custom(ref impl_fn) => {
                let span = impl_fn.span();
                Some(quote_spanned! {span=>
                    fn reflect_partial_eq(&self, value: &dyn #bevy_reflect_path::Reflect) -> #FQOption<bool> {
                        #FQOption::Some(#impl_fn(self, value))
                    }
                })
            }
            TraitImpl::NotImplemented => None,
        }
    }

    /// Returns the implementation of `Reflect::debug` as a `TokenStream`.
    ///
    /// If `Debug` was not registered, returns `None`.
    pub fn get_debug_impl(&self) -> Option<proc_macro2::TokenStream> {
        match &self.debug {
            &TraitImpl::Implemented(span) => Some(quote_spanned! {span=>
                fn debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    ::core::fmt::Debug::fmt(self, f)
                }
            }),
            TraitImpl::Custom(ref impl_fn) => {
                let span = impl_fn.span();
                Some(quote_spanned! {span=>
                    fn debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        #impl_fn(self, f)
                    }
                })
            }
            TraitImpl::NotImplemented => None,
        }
    }
}

/// Adds an identifier to a vector of identifiers if it is not already present.
///
/// Returns an error if the identifier already exists in the list.
fn add_unique_ident(idents: &mut Vec<Ident>, ident: Ident) -> Result<(), syn::Error> {
    let ident_name = ident.to_string();
    if idents.iter().any(|i| i == ident_name.as_str()) {
        return Err(syn::Error::new(ident.span(), CONFLICTING_TYPE_DATA_MESSAGE));
    }

    idents.push(ident);
    Ok(())
}
