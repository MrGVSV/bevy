use crate::func::signature::ArgumentSignature;
use crate::func::{ArgList, FunctionError, FunctionInfo, FunctionInfoType, FunctionOverloadError};
use alloc::borrow::Cow;
use bevy_utils::hashbrown::HashMap;
use core::ops::RangeInclusive;

/// A helper type for storing a mapping of overloaded functions
/// along with the corresponding [function information].
///
/// By using an enum, we can optimize the common case of a single function,
/// while still allowing for multiple function overloads to be stored.
///
/// [function information]: FunctionInfo
#[derive(Clone, Debug)]
pub(super) enum FunctionMap<F> {
    /// Represents a single, non-overloaded function.
    Single(F, FunctionInfo),
    /// Represents an overloaded function.
    Overloaded(
        /// The list of function overloads.
        Vec<F>,
        /// The information for each function.
        ///
        /// Note that some functions may have multiple `FunctionInfo` values (i.e. manually created overloads),
        /// so this list may not always line up one-to-one with the functions list.
        Vec<FunctionInfo>,
        /// A mapping of argument signatures to the index of the corresponding function.
        ///
        /// Multiple signatures may point to the same function index (i.e. for manually created overloads).
        HashMap<ArgumentSignature, usize>,
    ),
}

impl<F> FunctionMap<F> {
    /// Returns `true` if the map contains an overloaded function.
    pub fn is_overloaded(&self) -> bool {
        matches!(self, Self::Overloaded(..))
    }

    /// Get a reference to a function in the map.
    ///
    /// If there is only one function in the map, it will be returned.
    /// Otherwise, the function will be selected based on the arguments provided.
    ///
    /// If no overload matches the provided arguments, an error will be returned.
    pub fn get(&self, args: &ArgList) -> Result<&F, FunctionError> {
        match self {
            Self::Single(function, _) => Ok(function),
            Self::Overloaded(functions, _, indices) => {
                let signature = ArgumentSignature::from(args);
                indices
                    .get(&signature)
                    .map(|index| &functions[*index])
                    .ok_or_else(|| FunctionError::NoOverload {
                        expected: indices.keys().cloned().collect(),
                        received: signature,
                    })
            }
        }
    }

    /// Get a mutable reference to a function in the map.
    ///
    /// If there is only one function in the map, it will be returned.
    /// Otherwise, the function will be selected based on the arguments provided.
    ///
    /// If no overload matches the provided arguments, an error will be returned.
    pub fn get_mut(&mut self, args: &ArgList) -> Result<&mut F, FunctionError> {
        match self {
            Self::Single(function, _) => Ok(function),
            Self::Overloaded(functions, _, indices) => {
                let signature = ArgumentSignature::from(args);
                indices
                    .get(&signature)
                    .map(|index| &mut functions[*index])
                    .ok_or_else(|| FunctionError::NoOverload {
                        expected: indices.keys().cloned().collect(),
                        received: signature,
                    })
            }
        }
    }

    /// Returns the function information contained in the map.
    pub fn info(&self) -> FunctionInfoType {
        match self {
            Self::Single(_, info) => FunctionInfoType::Standard(Cow::Borrowed(info)),
            Self::Overloaded(_, info, _) => FunctionInfoType::Overloaded(Cow::Borrowed(info)),
        }
    }

    /// Returns the number of arguments the function expects.
    ///
    /// For [overloaded] functions that can have a variable number of arguments,
    /// this will return the minimum and maximum number of arguments.
    ///
    /// Otherwise, the range will have the same start and end.
    ///
    /// [overloaded]: Self::Overloaded
    pub fn arg_count(&self) -> RangeInclusive<usize> {
        self.info().arg_count()
    }

    /// Merge another [`FunctionMap`] into this one.
    ///
    /// If the other map contains any functions with the same signature as this one,
    /// an error will be returned along with the original, unchanged map.
    ///
    /// Therefore, this method should always return `Ok(Self::Overloaded(..))` if the merge is successful.
    ///
    /// Additionally, if the merge succeeds, it should be guaranteed that the order
    /// of the functions in the map will be preserved.
    /// For example, merging `[func_a, func_b]` (self) with `[func_c, func_d]` (other) should result in
    /// `[func_a, func_b, func_c, func_d]`.
    /// And merging `[func_c, func_d]` (self) with `[func_a, func_b]` (other) should result in
    /// `[func_c, func_d, func_a, func_b]`.
    pub fn merge(self, other: Self) -> Result<Self, (Box<Self>, FunctionOverloadError)> {
        match (self, other) {
            (Self::Single(self_func, self_info), Self::Single(other_func, other_info)) => {
                let self_sig = ArgumentSignature::from(&self_info);
                let other_sig = ArgumentSignature::from(&other_info);
                if self_sig == other_sig {
                    return Err((
                        Box::new(Self::Single(self_func, self_info)),
                        FunctionOverloadError {
                            signature: self_sig,
                        },
                    ));
                }

                let mut map = HashMap::new();
                map.insert_unique_unchecked(self_sig, 0);
                map.insert_unique_unchecked(other_sig, 1);

                Ok(Self::Overloaded(
                    vec![self_func, other_func],
                    vec![self_info, other_info],
                    map,
                ))
            }
            (
                Self::Single(self_func, self_info),
                Self::Overloaded(mut other_funcs, mut other_infos, mut other_indices),
            ) => {
                let self_sig = ArgumentSignature::from(&self_info);
                if other_indices.contains_key(&self_sig) {
                    return Err((
                        Box::new(Self::Single(self_func, self_info)),
                        FunctionOverloadError {
                            signature: self_sig,
                        },
                    ));
                }

                for index in other_indices.values_mut() {
                    *index += 1;
                }

                other_funcs.insert(0, self_func);
                other_infos.insert(0, self_info);
                other_indices.insert_unique_unchecked(self_sig, 0);

                Ok(Self::Overloaded(other_funcs, other_infos, other_indices))
            }
            (
                Self::Overloaded(mut self_funcs, mut self_infos, mut self_indices),
                Self::Single(other_func, other_info),
            ) => {
                let other_sig = ArgumentSignature::from(&other_info);
                if self_indices.contains_key(&other_sig) {
                    return Err((
                        Box::new(Self::Overloaded(self_funcs, self_infos, self_indices)),
                        FunctionOverloadError {
                            signature: other_sig,
                        },
                    ));
                }

                let index = self_funcs.len();
                self_funcs.push(other_func);
                self_infos.push(other_info);
                self_indices.insert_unique_unchecked(other_sig, index);

                Ok(Self::Overloaded(self_funcs, self_infos, self_indices))
            }
            (
                Self::Overloaded(mut self_funcs, mut self_infos, mut self_indices),
                Self::Overloaded(mut other_funcs, mut other_infos, other_indices),
            ) => {
                // Keep a separate map of the new indices to avoid mutating the existing one
                // until we can be sure the merge will be successful.
                let mut new_indices = HashMap::new();

                for (sig, index) in other_indices {
                    if self_indices.contains_key(&sig) {
                        return Err((
                            Box::new(Self::Overloaded(self_funcs, self_infos, self_indices)),
                            FunctionOverloadError { signature: sig },
                        ));
                    }

                    new_indices.insert_unique_unchecked(sig, self_funcs.len() + index);
                }

                self_indices.reserve(new_indices.len());
                for (sig, index) in new_indices {
                    self_indices.insert_unique_unchecked(sig, index);
                }

                self_funcs.append(&mut other_funcs);
                // The index map and `FunctionInfo` list should always be in sync,
                // so we can simply append the new infos to the existing ones.
                self_infos.append(&mut other_infos);

                Ok(Self::Overloaded(self_funcs, self_infos, self_indices))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::func::FunctionInfo;
    use crate::Type;

    #[test]
    fn should_merge_single_into_single() {
        let map_a = FunctionMap::Single('a', FunctionInfo::anonymous().with_arg::<i8>("arg0"));
        let map_b = FunctionMap::Single('b', FunctionInfo::anonymous().with_arg::<u8>("arg0"));

        let FunctionMap::Overloaded(functions, infos, indices) = map_a.merge(map_b).unwrap() else {
            panic!("expected overloaded function");
        };
        assert_eq!(functions, vec!['a', 'b']);
        assert_eq!(infos.len(), 2);
        assert_eq!(
            indices,
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 1),
            ])
        );
    }

    #[test]
    fn should_merge_single_into_overloaded() {
        let map_a = FunctionMap::Single('a', FunctionInfo::anonymous().with_arg::<i8>("arg0"));
        let map_b = FunctionMap::Overloaded(
            vec!['b', 'c'],
            vec![
                FunctionInfo::anonymous().with_arg::<u8>("arg0"),
                FunctionInfo::anonymous().with_arg::<u16>("arg0"),
            ],
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<u16>()]), 1),
            ]),
        );

        let FunctionMap::Overloaded(functions, infos, indices) = map_a.merge(map_b).unwrap() else {
            panic!("expected overloaded function");
        };
        assert_eq!(functions, vec!['a', 'b', 'c']);
        assert_eq!(infos.len(), 3);
        assert_eq!(
            indices,
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 1),
                (ArgumentSignature::from_iter([Type::of::<u16>()]), 2),
            ])
        );
    }

    #[test]
    fn should_merge_overloaed_into_single() {
        let map_a = FunctionMap::Overloaded(
            vec!['a', 'b'],
            vec![
                FunctionInfo::anonymous().with_arg::<i8>("arg0"),
                FunctionInfo::anonymous().with_arg::<i16>("arg0"),
            ],
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<i16>()]), 1),
            ]),
        );
        let map_b = FunctionMap::Single('c', FunctionInfo::anonymous().with_arg::<u8>("arg0"));

        let FunctionMap::Overloaded(functions, infos, indices) = map_a.merge(map_b).unwrap() else {
            panic!("expected overloaded function");
        };
        assert_eq!(functions, vec!['a', 'b', 'c']);
        assert_eq!(infos.len(), 3);
        assert_eq!(
            indices,
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<i16>()]), 1),
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 2),
            ])
        );
    }

    #[test]
    fn should_merge_overloaded_into_overloaded() {
        let map_a = FunctionMap::Overloaded(
            vec!['a', 'b'],
            vec![
                FunctionInfo::anonymous().with_arg::<i8>("arg0"),
                FunctionInfo::anonymous().with_arg::<i16>("arg0"),
            ],
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<i16>()]), 1),
            ]),
        );
        let map_b = FunctionMap::Overloaded(
            vec!['c', 'd'],
            vec![
                FunctionInfo::anonymous().with_arg::<u8>("arg0"),
                FunctionInfo::anonymous().with_arg::<u16>("arg0"),
            ],
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<u16>()]), 1),
            ]),
        );

        let FunctionMap::Overloaded(functions, infos, indices) = map_a.merge(map_b).unwrap() else {
            panic!("expected overloaded function");
        };
        assert_eq!(functions, vec!['a', 'b', 'c', 'd']);
        assert_eq!(infos.len(), 4);
        assert_eq!(
            indices,
            HashMap::from_iter([
                (ArgumentSignature::from_iter([Type::of::<i8>()]), 0),
                (ArgumentSignature::from_iter([Type::of::<i16>()]), 1),
                (ArgumentSignature::from_iter([Type::of::<u8>()]), 2),
                (ArgumentSignature::from_iter([Type::of::<u16>()]), 3),
            ])
        );
    }

    #[test]
    fn should_return_error_on_duplicate_signature() {
        let map_a = FunctionMap::Single(
            'a',
            FunctionInfo::anonymous()
                .with_arg::<i8>("arg0")
                .with_arg::<i16>("arg1"),
        );
        let map_b = FunctionMap::Overloaded(
            vec!['b', 'c'],
            vec![
                FunctionInfo::anonymous().with_arg::<u8>("arg0"),
                FunctionInfo::anonymous().with_arg::<u16>("arg1"),
            ],
            HashMap::from_iter([
                (
                    ArgumentSignature::from_iter([Type::of::<u8>(), Type::of::<u16>()]),
                    0,
                ),
                (
                    ArgumentSignature::from_iter([Type::of::<i8>(), Type::of::<i16>()]),
                    1,
                ),
            ]),
        );

        let (map, error) = map_a.merge(map_b).unwrap_err();
        assert_eq!(
            error.signature,
            ArgumentSignature::from_iter([Type::of::<i8>(), Type::of::<i16>()])
        );

        // Assert the original map remains unchanged:
        let FunctionMap::Single(function, info) = *map else {
            panic!("expected single function");
        };

        assert_eq!(function, 'a');
        assert_eq!(
            ArgumentSignature::from(info),
            ArgumentSignature::from_iter([Type::of::<i8>(), Type::of::<i16>()])
        );
    }
}
