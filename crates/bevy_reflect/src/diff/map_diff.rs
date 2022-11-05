use std::borrow::Cow;
use crate::diff::{Diff, DiffError, DiffResult, DiffType, ValueDiff};
use crate::{Map, Reflect, ReflectRef};
use std::fmt::{Debug, Formatter};
use std::slice::Iter;

/// Indicates the difference between two [`Map`] entries.
///
/// See the [module-level docs](crate::diff) for more details.
#[derive(Debug)]
pub enum MapDiff<'old, 'new> {
    /// An entry with the given key was removed.
    Deleted(ValueDiff<'old>),
    /// An entry with the given key and value was added.
    Inserted(ValueDiff<'new>, ValueDiff<'new>),
    /// The entry with the given key was modified.
    Modified(ValueDiff<'old>, Diff<'old, 'new>),
}

/// Diff object for [maps](Map).
pub struct DiffedMap<'old, 'new> {
    type_name: Cow<'new, str>,
    changes: Vec<MapDiff<'old, 'new>>,
}

impl<'old, 'new> DiffedMap<'old, 'new> {
    /// Returns the [type name] of the reflected value currently being diffed.
    ///
    /// [type name]: crate::Reflect::type_name
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the number of _changes_ made to the map.
    pub fn len_changes(&self) -> usize {
        self.changes.len()
    }

    /// Returns an iterator over the unordered sequence of edits needed to transform
    /// the "old" map into the "new" one.
    pub fn iter_changes(&self) -> Iter<'_, MapDiff<'old, 'new>> {
        self.changes.iter()
    }
}

impl<'old, 'new> Debug for DiffedMap<'old, 'new> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffedMap")
            .field("changes", &self.changes)
            .finish()
    }
}

/// Utility function for diffing two [`Map`] objects.
pub fn diff_map<'old, 'new, T: Map>(
    old: &'old T,
    new: &'new dyn Reflect,
) -> DiffResult<'old, 'new> {
    let new = match new.reflect_ref() {
        ReflectRef::Map(new) => new,
        _ => return Err(DiffError::ExpectedMap),
    };

    if old.type_name() != new.type_name() {
        return Ok(Diff::Replaced(ValueDiff::Borrowed(new.as_reflect())));
    }

    let mut diff = DiffedMap::<'old, 'new> {
        type_name: Cow::Borrowed(new.type_name()),
        changes: Vec::with_capacity(new.len()),
    };

    let mut was_modified = false;
    for (old_key, old_value) in old.iter() {
        if let Some(new_value) = new.get(old_key) {
            let value_diff = old_value.diff(new_value)?;
            if !matches!(value_diff, Diff::NoChange) {
                was_modified = true;
                diff.changes
                    .push(MapDiff::Modified(ValueDiff::Borrowed(old_key), value_diff));
            }
        } else {
            was_modified = true;
            diff.changes.push(MapDiff::Deleted(ValueDiff::Borrowed(old_key)));
        }
    }

    for (new_key, new_value) in new.iter() {
        if matches!(old.get(new_key), None) {
            was_modified = true;
            diff.changes.push(MapDiff::Inserted(
                ValueDiff::Borrowed(new_key),
                ValueDiff::Borrowed(new_value),
            ));
        }
    }

    if was_modified {
        Ok(Diff::Modified(DiffType::Map(diff)))
    } else {
        Ok(Diff::NoChange)
    }
}
