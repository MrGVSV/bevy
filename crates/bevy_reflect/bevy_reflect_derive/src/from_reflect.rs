use crate::ReflectDeriveData;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Field, Generics, Ident, Index, Member, Path};

/// Implements `FromReflect` for the given struct
pub fn impl_struct(derive_data: &ReflectDeriveData) -> TokenStream {
    impl_struct_internal(derive_data, false)
}

/// Implements `FromReflect` for the given tuple struct
pub fn impl_tuple_struct(derive_data: &ReflectDeriveData) -> TokenStream {
    impl_struct_internal(derive_data, true)
}

/// Implements `FromReflect` for the given value type
pub fn impl_value(type_name: &Ident, generics: &Generics, bevy_reflect_path: &Path) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    TokenStream::from(quote! {
        impl #impl_generics #bevy_reflect_path::FromReflect for #type_name #ty_generics #where_clause  {
            fn from_reflect(reflect: &dyn #bevy_reflect_path::Reflect) -> Option<Self> {
                Some(reflect.any().downcast_ref::<#type_name #ty_generics>()?.clone())
            }
        }
    })
}

/// Container for a struct's members (field name or index) and their
/// corresponding values.
struct StructFields(Vec<Member>, Vec<proc_macro2::TokenStream>);

impl StructFields {
    pub fn new(items: (Vec<Member>, Vec<proc_macro2::TokenStream>)) -> Self {
        Self(items.0, items.1)
    }
}

fn impl_struct_internal(derive_data: &ReflectDeriveData, is_tuple: bool) -> TokenStream {
    let struct_name = derive_data.type_name();
    let generics = derive_data.generics();
    let bevy_reflect_path = derive_data.bevy_reflect_path();

    let ref_struct = Ident::new("__ref_struct", Span::call_site());
    let ref_struct_type = if is_tuple {
        Ident::new("TupleStruct", Span::call_site())
    } else {
        Ident::new("Struct", Span::call_site())
    };

    let field_types = derive_data.active_types();
    let StructFields(ignored_members, ignored_values) = get_ignored_fields(derive_data, is_tuple);
    let StructFields(active_members, active_values) =
        get_active_fields(derive_data, &ref_struct, is_tuple);

    let default_ident = Ident::new("ReflectDefault", Span::call_site());
    let constructor = if derive_data.attrs().data().contains(&default_ident) {
        quote!(
            let mut __this = Self::default();
            #(
                __this.#active_members = #active_values;
            )*
            Some(__this)
        )
    } else {
        quote!(
            Some(
                Self {
                    #(#active_members: #active_values,)*
                    #(#ignored_members: #ignored_values,)*
                }
            )
        )
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Add FromReflect bound for each active field
    let mut where_from_reflect_clause = if where_clause.is_some() {
        quote! {#where_clause}
    } else if !active_members.is_empty() {
        quote! {where}
    } else {
        quote! {}
    };
    where_from_reflect_clause.extend(quote! {
        #(#field_types: #bevy_reflect_path::FromReflect,)*
    });

    TokenStream::from(quote! {
        impl #impl_generics #bevy_reflect_path::FromReflect for #struct_name #ty_generics #where_from_reflect_clause
        {
            fn from_reflect(reflect: &dyn #bevy_reflect_path::Reflect) -> Option<Self> {
                use #bevy_reflect_path::#ref_struct_type;
                if let #bevy_reflect_path::ReflectRef::#ref_struct_type(#ref_struct) = reflect.reflect_ref() {
                    #constructor
                } else {
                    None
                }
            }
        }
    })
}

/// Get the collection of ignored field definitions
///
/// Each item in the collection takes the form: `field_ident: field_value`.
fn get_ignored_fields(derive_data: &ReflectDeriveData, is_tuple: bool) -> StructFields {
    StructFields::new(
        derive_data
            .ignored_fields()
            .map(|(field, _attr, index)| {
                let member = get_ident(field, *index, is_tuple);
                let value = quote! {
                    Default::default()
                };

                (member, value)
            })
            .unzip(),
    )
}

/// Get the collection of active field definitions
///
/// Each item in the collection takes the form: `field_ident: field_value`.
fn get_active_fields(
    derive_data: &ReflectDeriveData,
    dyn_struct_name: &Ident,
    is_tuple: bool,
) -> StructFields {
    let bevy_reflect_path = derive_data.bevy_reflect_path();

    StructFields::new(
        derive_data
            .active_fields()
            .map(|(field, _attr, index)| {
                let member = get_ident(field, *index, is_tuple);
                let ty = field.ty.clone();

                // Accesses the field on the given dynamic struct or tuple struct
                let get_field = if is_tuple {
                    quote! {
                        #dyn_struct_name.field(#index)
                    }
                } else {
                    let name = field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| index.to_string());
                    quote! {
                        #dyn_struct_name.field(#name)
                    }
                };

                let value = quote! { {
                    <#ty as #bevy_reflect_path::FromReflect>::from_reflect(#get_field?)?
                }};

                (member, value)
            })
            .unzip(),
    )
}

fn get_ident(field: &Field, index: usize, is_tuple: bool) -> Member {
    if is_tuple {
        Member::Unnamed(Index::from(index))
    } else {
        field
            .ident
            .as_ref()
            .map(|ident| Member::Named(ident.clone()))
            .unwrap_or_else(|| Member::Unnamed(Index::from(index)))
    }
}
