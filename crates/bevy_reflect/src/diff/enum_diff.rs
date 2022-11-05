use crate::diff::{Diff, DiffError, DiffResult, DiffType, DiffedStruct, DiffedTuple, ValueDiff};
use crate::{Enum, Reflect, ReflectRef, VariantType};
use std::borrow::Cow;
use std::fmt::Debug;

/// Contains diffing details for [enums](crate::Enum).
#[derive(Debug)]
pub enum EnumDiff<'old, 'new> {
    /// Functionally similar to [`Diff::Replaced`], but for variants within the same enum.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_reflect::{Reflect, diff::{Diff, DiffType, EnumDiff}};
    /// let old: Option<i32> = Some(123);
    /// let new: Option<i32> = None;
    ///
    /// let diff = old.diff(&new).unwrap();
    /// assert!(matches!(diff, Diff::Modified(DiffType::Enum(EnumDiff::Swapped(..)))));
    /// ```
    ///
    Swapped(ValueDiff<'new>),
    Tuple(DiffedTuple<'old, 'new>),
    Struct(DiffedStruct<'old, 'new>),
}

impl<'old, 'new> EnumDiff<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        match self {
            Self::Swapped(value_diff) => value_diff.type_name(),
            Self::Tuple(tuple_diff) => tuple_diff.type_name(),
            Self::Struct(struct_diff) => struct_diff.type_name(),
        }
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

    if old.type_name() != new.type_name() {
        return Ok(Diff::Replaced(ValueDiff::Borrowed(new.as_reflect())));
    }

    if old.variant_type() != new.variant_type() || old.variant_name() != new.variant_name() {
        return Ok(Diff::Modified(DiffType::Enum(EnumDiff::Swapped(
            ValueDiff::Borrowed(new.as_reflect()),
        ))));
    }

    let diff = match old.variant_type() {
        VariantType::Struct => {
            let mut diff = DiffedStruct::new(new.type_name(), new.field_len());

            let mut was_modified = false;
            for old_field in old.iter_fields() {
                let field_name = old_field.name().unwrap();
                let new_field = new.field(field_name).ok_or(DiffError::MissingField)?;
                let field_diff = old_field.value().diff(new_field)?;
                was_modified |= !matches!(field_diff, Diff::NoChange);
                diff.push(Cow::Borrowed(field_name), field_diff);
            }

            if was_modified {
                Diff::Modified(DiffType::Enum(EnumDiff::Struct(diff)))
            } else {
                Diff::NoChange
            }
        }
        VariantType::Tuple => {
            let mut diff = DiffedTuple::new(new.type_name(), new.field_len());

            let mut was_modified = false;
            for (old_field, new_field) in old.iter_fields().zip(new.iter_fields()) {
                let field_diff = old_field.value().diff(new_field.value())?;
                was_modified |= !matches!(field_diff, Diff::NoChange);
                diff.push(field_diff);
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
