use crate::diff::{Diff, DiffError, DiffResult, DiffType, ValueDiff};
use crate::{Array, Reflect, ReflectRef};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::slice::Iter;

/// Diff object for [arrays](Array).
pub struct DiffedArray<'old, 'new> {
    type_name: Cow<'new, str>,
    elements: Vec<Diff<'old, 'new>>,
}

impl<'old, 'new> DiffedArray<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the [`Diff`] for the field at the given index.
    pub fn get(&self, index: usize) -> Option<&Diff<'old, 'new>> {
        self.elements.get(index)
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns true if the array contains no elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Returns an iterator over the [`Diff`] for _every_ element.
    pub fn iter(&self) -> Iter<'_, Diff<'old, 'new>> {
        self.elements.iter()
    }
}

impl<'old, 'new> Debug for DiffedArray<'old, 'new> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffedArray")
            .field("elements", &self.elements)
            .finish()
    }
}

/// Utility function for diffing two [`Array`] objects.
pub fn diff_array<'old, 'new, T: Array>(
    old: &'old T,
    new: &'new dyn Reflect,
) -> DiffResult<'old, 'new> {
    let new = match new.reflect_ref() {
        ReflectRef::Array(new) => new,
        _ => return Err(DiffError::ExpectedArray),
    };

    if old.len() != new.len() || old.type_name() != new.type_name() {
        return Ok(Diff::Replaced(ValueDiff::Borrowed(new.as_reflect())));
    }

    let mut diff = DiffedArray {
        type_name: Cow::Borrowed(new.type_name()),
        elements: Vec::with_capacity(old.len()),
    };

    let mut was_modified = false;
    for (old_field, new_field) in old.iter().zip(new.iter()) {
        let field_diff = old_field.diff(new_field)?;
        was_modified |= !matches!(field_diff, Diff::NoChange);
        diff.elements.push(field_diff);
    }

    if was_modified {
        Ok(Diff::Modified(DiffType::Array(diff)))
    } else {
        Ok(Diff::NoChange)
    }
}
