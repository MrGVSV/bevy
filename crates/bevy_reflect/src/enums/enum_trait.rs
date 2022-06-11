use crate::{Reflect, ReflectRef, Struct, Tuple, VariantInfo, VariantType};
use bevy_utils::HashMap;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::slice::Iter;

pub trait Enum: Reflect {
    /// Returns a reference to the value of the field (in the current variant) with the given name.
    fn field(&self, name: &str) -> Option<&dyn Reflect>;
    /// Returns a reference to the value of the field (in the current variant) at the given index.
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;
    /// Returns a mutable reference to the value of the field (in the current variant) with the given name.
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect>;
    /// Returns a mutable reference to the value of the field (in the current variant) at the given index.
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;
    /// Returns the index of the field (in the current variant) with the given name.
    fn index_of(&self, name: &str) -> Option<usize>;
    /// Returns an iterator over the values of the current variant's fields.
    fn iter_fields(&self) -> VariantFieldIter;
    /// Returns the number of fields in the current variant.
    fn field_len(&self) -> usize;
    /// The name of the current variant.
    fn variant_name(&self) -> &str;
    /// The type of the current variant.
    fn variant_type(&self) -> VariantType;
    /// Returns true if the current variant's type matches the given one.
    fn is_variant(&self, variant_type: VariantType) -> bool {
        self.variant_type() == variant_type
    }
}

/// A container for compile-time enum info.
#[derive(Clone, Debug)]
pub struct EnumInfo {
    type_name: &'static str,
    type_id: TypeId,
    variants: Box<[VariantInfo]>,
    variant_indices: HashMap<Cow<'static, str>, usize>,
}

impl EnumInfo {
    /// Create a new [`EnumInfo`].
    ///
    /// # Arguments
    ///
    /// * `variants`: The variants of this enum in the order they are defined
    ///
    pub fn new<TEnum: Enum>(variants: &[VariantInfo]) -> Self {
        let variant_indices = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let name = variant.name().clone();
                (name, index)
            })
            .collect::<HashMap<_, _>>();

        Self {
            type_name: std::any::type_name::<TEnum>(),
            type_id: TypeId::of::<TEnum>(),
            variants: variants.to_vec().into_boxed_slice(),
            variant_indices,
        }
    }

    /// Get a variant with the given name.
    pub fn variant(&self, name: &str) -> Option<&VariantInfo> {
        self.variant_indices
            .get(name)
            .map(|index| &self.variants[*index])
    }

    /// Get a variant at the given index.
    pub fn variant_at(&self, index: usize) -> Option<&VariantInfo> {
        self.variants.get(index)
    }

    /// Get the index of the variant with the given name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.variant_indices.get(name).copied()
    }

    /// Iterate over the variants of this enum.
    pub fn iter(&self) -> Iter<'_, VariantInfo> {
        self.variants.iter()
    }

    /// The number of variants in this enum.
    pub fn variant_len(&self) -> usize {
        self.variants.len()
    }

    /// The [type name] of the enum.
    ///
    /// [type name]: std::any::type_name
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The [`TypeId`] of the enum.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Check if the given type matches the enum type.
    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }
}

pub struct VariantFieldIter<'a> {
    pub(crate) container: &'a dyn Enum,
    pub(crate) index: usize,
}

impl<'a> VariantFieldIter<'a> {
    pub fn new(container: &'a dyn Enum) -> Self {
        Self {
            container,
            index: 0,
        }
    }
}

impl<'a> Iterator for VariantFieldIter<'a> {
    type Item = &'a dyn Reflect;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.container.field_at(self.index);
        self.index += 1;
        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.container.field_len();
        (size, Some(size))
    }
}

impl<'a> ExactSizeIterator for VariantFieldIter<'a> {}

#[inline]
pub fn enum_partial_eq<E: Enum>(enum_a: &E, reflect_b: &dyn Reflect) -> Option<bool> {
    // TODO: Uncomment and update once we figure out how we want to represent variants
    // let enum_b = if let ReflectRef::Enum(e) = reflect_b.reflect_ref() {
    //     e
    // } else {
    //     return Some(false);
    // };
    //
    // if enum_a.variant_info() != enum_b.variant_info() {
    //     return Some(false);
    // }
    //
    // let variant_b = enum_b.variant();
    // match enum_a.variant() {
    //     EnumVariant::Unit => {
    //         if let EnumVariant::Unit = variant_b {
    //         } else {
    //             return Some(false);
    //         }
    //     }
    //     EnumVariant::NewType(t_a) => {
    //         if let EnumVariant::NewType(t_b) = variant_b {
    //             if let Some(false) | None = t_b.reflect_partial_eq(t_a) {
    //                 return Some(false);
    //             }
    //         } else {
    //             return Some(false);
    //         }
    //     }
    //     EnumVariant::Tuple(t_a) => {
    //         if let EnumVariant::Tuple(t_b) = variant_b {
    //             if let Some(false) | None = t_b.reflect_partial_eq(t_a.as_reflect()) {
    //                 return Some(false);
    //             }
    //         } else {
    //             return Some(false);
    //         }
    //     }
    //     EnumVariant::Struct(s_a) => {
    //         if let EnumVariant::Struct(s_b) = variant_b {
    //             if let Some(false) | None = s_b.reflect_partial_eq(s_a.as_reflect()) {
    //                 return Some(false);
    //             }
    //         } else {
    //             return Some(false);
    //         }
    //     }
    // }
    Some(true)
}
