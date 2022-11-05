use crate::Reflect;
use std::ops::Deref;

/// Represents a plain value in a [`Diff`](crate::diff::Diff).
///
/// This can contain either an owned [`Reflect`] object or an immutable/mutable reference to one.
#[derive(Debug)]
pub enum ValueDiff<'a> {
    Borrowed(&'a dyn Reflect),
    Owned(Box<dyn Reflect>),
}

impl<'a> Deref for ValueDiff<'a> {
    type Target = dyn Reflect;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(value) => *value,
            Self::Owned(value) => value.as_ref(),
        }
    }
}

impl<'a> From<&'a dyn Reflect> for ValueDiff<'a> {
    fn from(value: &'a dyn Reflect) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a> From<Box<dyn Reflect>> for ValueDiff<'a> {
    fn from(value: Box<dyn Reflect>) -> Self {
        Self::Owned(value)
    }
}
