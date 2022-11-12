use crate::diff::{
    DiffApplyError, DiffError, DiffedArray, DiffedList, DiffedMap, DiffedStruct, DiffedTuple,
    DiffedTupleStruct, EnumDiff, ValueDiff,
};
use crate::{Array, Enum, List, Map, Reflect, ReflectOwned, Struct, Tuple, TupleStruct};

/// Indicates the difference between two [`Reflect`] objects.
///
/// [`Reflect`]: crate::Reflect
#[derive(Debug)]
pub enum Diff<'old, 'new> {
    /// Indicates no change.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_reflect::{Reflect, diff::Diff};
    /// let old = 123;
    /// let new = 123;
    ///
    /// let diff = old.diff(&new).unwrap();
    /// assert!(matches!(diff, Diff::NoChange));
    /// ```
    ///
    NoChange,
    /// Indicates that the type has been changed.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_reflect::{Reflect, diff::Diff};
    /// let old: bool = true;
    /// let new: i32 = 123;
    ///
    /// let diff = old.diff(&new).unwrap();
    /// assert!(matches!(diff, Diff::Replaced(..)));
    /// ```
    ///
    Replaced(ValueDiff<'new>),
    /// Indicates that the value has been modified.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_reflect::{Reflect, diff::Diff};
    /// let old: i32 = 123;
    /// let new: i32 = 456;
    ///
    /// let diff = old.diff(&new).unwrap();
    /// assert!(matches!(diff, Diff::Modified(..)));
    /// ```
    ///
    Modified(DiffType<'old, 'new>),
}

impl<'old, 'new> Diff<'old, 'new> {
    /// Apply this `Diff` to the given [`Reflect`] object.
    ///
    /// Returns the updated `Reflect` object if successful.
    /// Otherwise, returns a [`DiffApplyError`].
    pub fn apply(self, base: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, DiffApplyError> {
        let diff = match self {
            Diff::NoChange => return Ok(base),
            Diff::Replaced(ValueDiff::Owned(value)) => return Ok(value),
            Diff::Replaced(ValueDiff::Borrowed(value)) => return Ok(value.clone_value()),
            Diff::Modified(diff_type) => diff_type,
        };

        let base = base.reflect_owned();

        match (base, diff) {
            // === Value === //
            (ReflectOwned::Value(_), DiffType::Value(ValueDiff::Owned(value))) => Ok(value),
            (ReflectOwned::Value(_), DiffType::Value(ValueDiff::Borrowed(value))) => {
                Ok(value.clone_value())
            }
            (_, DiffType::Value(_)) => Err(DiffApplyError::ExpectedValue),
            // === Tuple === //
            (ReflectOwned::Tuple(value), DiffType::Tuple(diff)) => {
                Tuple::apply_tuple_diff(value, diff)
            }
            (_, DiffType::Tuple(_)) => Err(DiffApplyError::ExpectedTuple),
            // === Array === //
            (ReflectOwned::Array(value), DiffType::Array(diff)) => {
                Array::apply_array_diff(value, diff)
            }
            (_, DiffType::Array(_)) => Err(DiffApplyError::ExpectedArray),
            // === List === //
            (ReflectOwned::List(value), DiffType::List(diff)) => List::apply_list_diff(value, diff),
            (_, DiffType::List(_)) => Err(DiffApplyError::ExpectedList),
            // === Map === //
            (ReflectOwned::Map(value), DiffType::Map(diff)) => Map::apply_map_diff(value, diff),
            (_, DiffType::Map(_)) => Err(DiffApplyError::ExpectedMap),
            // === Tuple Struct === //
            (ReflectOwned::TupleStruct(value), DiffType::TupleStruct(diff)) => {
                TupleStruct::apply_tuple_struct_diff(value, diff)
            }
            (_, DiffType::TupleStruct(_)) => Err(DiffApplyError::ExpectedTupleStruct),
            // === Struct === //
            (ReflectOwned::Struct(value), DiffType::Struct(diff)) => {
                Struct::apply_struct_diff(value, diff)
            }
            (_, DiffType::Struct(_)) => Err(DiffApplyError::ExpectedStruct),
            // === Enum === //
            (ReflectOwned::Enum(value), DiffType::Enum(diff)) => Enum::apply_enum_diff(value, diff),
            (_, DiffType::Enum(_)) => Err(DiffApplyError::ExpectedEnum),
        }
    }
}

/// Contains diffing details for each [reflection type].
///
/// [reflection type]: crate::ReflectRef
#[derive(Debug)]
pub enum DiffType<'old, 'new> {
    Value(ValueDiff<'new>),
    Tuple(DiffedTuple<'old, 'new>),
    Array(DiffedArray<'old, 'new>),
    List(DiffedList<'new>),
    Map(DiffedMap<'old, 'new>),
    TupleStruct(DiffedTupleStruct<'old, 'new>),
    Struct(DiffedStruct<'old, 'new>),
    Enum(EnumDiff<'old, 'new>),
}

impl<'old, 'new> DiffType<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        match self {
            DiffType::Value(value_diff) => value_diff.type_name(),
            DiffType::Tuple(tuple_diff) => tuple_diff.type_name(),
            DiffType::Array(array_diff) => array_diff.type_name(),
            DiffType::List(list_diff) => list_diff.type_name(),
            DiffType::Map(map_diff) => map_diff.type_name(),
            DiffType::TupleStruct(tuple_struct_diff) => tuple_struct_diff.type_name(),
            DiffType::Struct(struct_diff) => struct_diff.type_name(),
            DiffType::Enum(enum_diff) => enum_diff.type_name(),
        }
    }
}

/// Alias for a `Result` that returns either [`Ok(Diff)`](Diff) or [`Err(DiffError)`](DiffError).
///
/// This is most commonly used by the [`Reflect::diff`] method as well as the utility functions
/// provided in this module.
///
/// [`Reflect::diff`]: crate::Reflect::diff
pub type DiffResult<'old, 'new> = Result<Diff<'old, 'new>, DiffError>;
