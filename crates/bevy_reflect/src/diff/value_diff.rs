use crate::diff::{Diff, DiffError, DiffResult, DiffType};
use crate::Reflect;

/// Utility function for diffing two [`Reflect`] objects.
///
/// This should be used for [value] types such as primitives.
/// For structs, enums, and other data structures, see the similar methods in the [diff] module.
///
/// [value]: crate::ReflectRef::Value
/// [diff]: crate::diff
pub fn diff_value<'old, 'new>(
    old: &'old dyn Reflect,
    new: &'new dyn Reflect,
) -> DiffResult<'old, 'new> {
    if old.type_name() != new.type_name() {
        return Ok(Diff::Replaced(new));
    }

    match old.reflect_partial_eq(new) {
        Some(true) => Ok(Diff::NoChange(old)),
        Some(false) => Ok(Diff::Modified(DiffType::Value(new))),
        None => Err(DiffError::Incomparable),
    }
}
