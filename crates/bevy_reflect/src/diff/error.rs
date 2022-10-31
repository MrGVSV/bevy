use thiserror::Error;

/// Error enum used when diffing two [`Reflect`](crate::Reflect) objects.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum DiffError {
    #[error("expected tuple, but found a different reflect value")]
    ExpectedTuple,
    #[error("expected array, but found a different reflect value")]
    ExpectedArray,
    #[error("expected list, but found a different reflect value")]
    ExpectedList,
    #[error("expected map, but found a different reflect value")]
    ExpectedMap,
    #[error("expected tuple struct, but found a different reflect value")]
    ExpectedTupleStruct,
    #[error("expected struct, but found a different reflect value")]
    ExpectedStruct,
    #[error("expected enum, but found a different reflect value")]
    ExpectedEnum,
    #[error("expected a required field")]
    MissingField,
    #[error("the given values cannot be compared")]
    Incomparable,
}
