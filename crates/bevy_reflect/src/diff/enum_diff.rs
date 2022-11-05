use crate::diff::{Diff, DiffError, DiffResult, DiffType, ValueDiff};
use crate::{Enum, Reflect, ReflectRef, VariantType};
use bevy_utils::HashMap;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::slice::Iter;

/// Contains diffing details for [tuple](crate::VariantType::Tuple)
/// and [struct](crate::VariantType::Struct) enum variants.
///
/// This does not contain details for [unit](crate::VariantType::Unit) variants as those are completely
/// handled by both [`Diff::NoChange`] and [`Diff::Replaced`].
#[derive(Debug)]
pub enum EnumDiff<'old, 'new> {
    Tuple(DiffedTupleVariant<'old, 'new>),
    Struct(DiffedStructVariant<'old, 'new>),
}

impl<'old, 'new> EnumDiff<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        match self {
            EnumDiff::Tuple(tuple_variant_diff) => tuple_variant_diff.type_name(),
            EnumDiff::Struct(struct_variant_diff) => struct_variant_diff.type_name(),
        }
    }
}

/// Diff object for [tuple variants](crate::VariantType::Tuple).
pub struct DiffedTupleVariant<'old, 'new> {
    type_name: Cow<'new, str>,
    fields: Vec<Diff<'old, 'new>>,
}

impl<'old, 'new> DiffedTupleVariant<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the [`Diff`] for the field at the given index.
    pub fn field(&self, index: usize) -> Option<&Diff<'old, 'new>> {
        self.fields.get(index)
    }

    /// Returns the number of fields in the tuple variant.
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }

    /// Returns an iterator over the [`Diff`] for _every_ field.
    pub fn field_iter(&self) -> Iter<'_, Diff<'old, 'new>> {
        self.fields.iter()
    }
}

impl<'old, 'new> Debug for DiffedTupleVariant<'old, 'new> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffedTupleVariant")
            .field("fields", &self.fields)
            .finish()
    }
}

/// Diff object for [struct variants](crate::VariantType::Struct).
pub struct DiffedStructVariant<'old, 'new> {
    type_name: Cow<'new, str>,
    fields: HashMap<Cow<'old, str>, Diff<'old, 'new>>,
    field_order: Vec<Cow<'old, str>>,
}

impl<'old, 'new> DiffedStructVariant<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the [`Diff`] for the field with the given name.
    pub fn field(&self, name: &str) -> Option<&Diff<'old, 'new>> {
        self.fields.get(name)
    }

    /// Returns the [`Diff`] for the field at the given index.
    pub fn field_at(&self, index: usize) -> Option<&Diff<'old, 'new>> {
        self.field_order
            .get(index)
            .and_then(|name| self.fields.get(name))
    }

    /// Returns the number of fields in the struct variant.
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }

    /// Returns an iterator over the name and [`Diff`] for _every_ field.
    pub fn field_iter(&self) -> impl Iterator<Item = (&'_ str, &'_ Diff<'old, 'new>)> {
        self.field_order
            .iter()
            .map(|name| (name.as_ref(), self.fields.get(name).unwrap()))
    }
}

impl<'old, 'new> Debug for DiffedStructVariant<'old, 'new> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffedStructVariant")
            .field("fields", &self.fields)
            .finish()
    }
}

/// Utility function for diffing two [`Enum`] objects.
pub fn diff_enum<'old, 'new, T: Enum>(
    old: &'old T,
    new: &'new dyn Reflect,
) -> DiffResult<'old, 'new> {
    let new = match new.reflect_ref() {
        ReflectRef::Enum(new) => new,
        _ => return Err(DiffError::ExpectedEnum),
    };

    if old.variant_type() != new.variant_type()
        || old.variant_name() != new.variant_name()
        || old.type_name() != new.type_name()
    {
        return Ok(Diff::Replaced(ValueDiff::Borrowed(new.as_reflect())));
    }

    let diff = match old.variant_type() {
        VariantType::Struct => {
            let mut diff = DiffedStructVariant {
                type_name: Cow::Borrowed(new.type_name()),
                fields: HashMap::with_capacity(new.field_len()),
                field_order: Vec::with_capacity(new.field_len()),
            };

            let mut was_modified = false;
            for old_field in old.iter_fields() {
                let field_name = old_field.name().unwrap();
                let new_field = new.field(field_name).ok_or(DiffError::MissingField)?;
                let field_diff = old_field.value().diff(new_field)?;
                was_modified |= !matches!(field_diff, Diff::NoChange);
                diff.fields.insert(Cow::Borrowed(field_name), field_diff);
                diff.field_order.push(Cow::Borrowed(field_name));
            }

            if was_modified {
                Diff::Modified(DiffType::Enum(EnumDiff::Struct(diff)))
            } else {
                Diff::NoChange
            }
        }
        VariantType::Tuple => {
            let mut diff = DiffedTupleVariant {
                type_name: Cow::Borrowed(new.type_name()),
                fields: Vec::with_capacity(old.field_len()),
            };

            let mut was_modified = false;
            for (old_field, new_field) in old.iter_fields().zip(new.iter_fields()) {
                let field_diff = old_field.value().diff(new_field.value())?;
                was_modified |= !matches!(field_diff, Diff::NoChange);
                diff.fields.push(field_diff);
            }

            if was_modified {
                Diff::Modified(DiffType::Enum(EnumDiff::Tuple(diff)))
            } else {
                Diff::NoChange
            }
        }
        VariantType::Unit => Diff::NoChange,
    };

    Ok(diff)
}
