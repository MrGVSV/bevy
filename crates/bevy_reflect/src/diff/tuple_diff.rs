use crate::diff::{Diff, DiffError, DiffResult, DiffType, ValueDiff};
use crate::{Reflect, ReflectRef, Tuple};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::slice::Iter;

/// Diff object for (tuples)[Tuple].
pub struct DiffedTuple<'old, 'new> {
    type_name: Cow<'new, str>,
    fields: Vec<Diff<'old, 'new>>,
}

impl<'old, 'new> DiffedTuple<'old, 'new> {
    pub(crate) fn new(type_name: &'new str, field_len: usize) -> Self {
        Self {
            type_name: Cow::Borrowed(type_name),
            fields: Vec::with_capacity(field_len),
        }
    }

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

    /// Returns the number of fields in the tuple.
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }

    /// Returns an iterator over the [`Diff`] for _every_ field.
    pub fn field_iter(&self) -> Iter<'_, Diff<'old, 'new>> {
        self.fields.iter()
    }

    pub(crate) fn push(&mut self, field_diff: Diff<'old, 'new>) {
        self.fields.push(field_diff);
    }
}

impl<'old, 'new> Debug for DiffedTuple<'old, 'new> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffedTuple")
            .field("fields", &self.fields)
            .finish()
    }
}

/// Utility function for diffing two [`Tuple`] objects.
pub fn diff_tuple<'old, 'new, T: Tuple>(
    old: &'old T,
    new: &'new dyn Reflect,
) -> DiffResult<'old, 'new> {
    let new = match new.reflect_ref() {
        ReflectRef::Tuple(new) => new,
        _ => return Err(DiffError::ExpectedTuple),
    };

    if old.field_len() != new.field_len() || old.type_name() != new.type_name() {
        return Ok(Diff::Replaced(ValueDiff::Borrowed(new.as_reflect())));
    }

    let mut diff = DiffedTuple::new(new.type_name(), new.field_len());

    let mut was_modified = false;
    for (old_field, new_field) in old.iter_fields().zip(new.iter_fields()) {
        let field_diff = old_field.diff(new_field)?;
        was_modified |= !matches!(field_diff, Diff::NoChange);
        diff.push(field_diff);
    }

    if was_modified {
        Ok(Diff::Modified(DiffType::Tuple(diff)))
    } else {
        Ok(Diff::NoChange)
    }
}
