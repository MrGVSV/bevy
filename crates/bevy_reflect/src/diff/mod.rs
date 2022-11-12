//! Tools for diffing two [`Reflect`] objects.
//!
//! The core of diffing revolves around the [`Diff`] and [`DiffType`] enums.
//! With these enums, diffs can be generated recursively for all reflect types.
//!
//! When diffing, the two objects are often referred to as "old" and "new".
//! This use of this particular language is purely for clarity's sake and does not necessarily
//! indicate that the "old" value is to be replaced by the "new" one.
//! These terms better indicate the directionality of the diffing operations,
//! which asks _"how can we transform `old` into `new`?"_
//! Other terms include "src" and "dst" as well as "a" and "b".
//!
//! To compute the diff between two objects, use the [`Reflect::diff`] method.
//! This will return the [`Diff`] or an error if diffing failed.
//!
//! With this, we can determine whether a value was [modified], [replaced], or had [no change].
//! When a value is [modified], it contains data related to the modification,
//! which may recursively contain more [`Diff`] objects.
//!
//! # Lists & Maps
//!
//! It's important to note that both [List](crate::List) and [Map](crate::Map) types work a bit differently
//! than the other types.
//! This is due to the fact that their size fields are not known at compile time.
//! For example, a list can grow and shrink dynamically, and a map can add or remove entries just as easily.
//!
//! This means there has to be a better approach to representing their diffs that take such factors
//! into account.
//!
//! ## Lists
//!
//! [Lists](crate::List) are diffed using the [Myers Diffing Algorithm].
//! Instead of diffing elements individually in sequence, we try to find the minimum number of edits
//! to transform the "old" list into the "new" one.
//!
//! The available edits are [`ListDiff::Inserted`] and [`ListDiff::Deleted`].
//! When calling [`DiffedList::iter_changes`], we iterate over a collection of these edits.
//! Each edit is given an index to determine where the transformation should take place in the "old" list.
//! [`ListDiff::Deleted`] edits are given the index of the element to delete,
//! while [`ListDiff::Inserted`] edits are given both the index of the element they should appear _before_
//! as well as the actual data to insert.
//!
//! Note: Multiple inserts may share the same index.
//! This is because, as far as each insertion is concerned, they all come before the element in the
//! "old" list at that index.
//!
//! ```
//! # use bevy_reflect::{Reflect, diff::{Diff, DiffType, ListDiff}};
//! let old = vec![8, -1, 5];
//! let new = vec![9, 8, 7, 6, 5];
//!
//! let diff = old.diff(&new).unwrap();
//!
//! if let Diff::Modified(DiffType::List(list_diff)) = diff {
//!   let mut changes = list_diff.iter_changes();
//!
//!   assert!(matches!(changes.next(), Some(ListDiff::Inserted(0, _))));
//!   assert!(matches!(changes.next(), Some(ListDiff::Deleted(1))));
//!   assert!(matches!(changes.next(), Some(ListDiff::Inserted(2, _))));
//!   assert!(matches!(changes.next(), Some(ListDiff::Inserted(2, _))));
//!   assert!(matches!(changes.next(), None));
//! }
//! ```
//!
//! ## Maps
//!
//! [Maps](crate::Map) also include edits for [insertion](`MapDiff::Inserted`) and [deletion](MapDiff::Deleted),
//! but contain a third option: [`MapDiff::Modified`].
//! Unlike lists, these edits are unordered and do not make use of the [Myers Diffing Algorithm].
//! Instead, the [`MapDiff::Inserted`] and [`MapDiff::Deleted`] edits simply indicate whether an entry with a given
//! key was inserted or deleted,
//! while the [`MapDiff::Modified`] edit indicates that the _value_ of an entry was edited.
//!
//! ```
//! # use bevy_reflect::{Reflect, diff::{Diff, DiffType, MapDiff}};
//! # use bevy_utils::HashMap;
//! let old = HashMap::from([(1, 111), (2, 222), (3, 333)]);
//! let new = HashMap::from([(2, 999), (3, 333), (4, 444)]);
//!
//! let diff = old.diff(&new).unwrap();
//!
//! if let Diff::Modified(DiffType::Map(map_diff)) = diff {
//!   let mut deleted_1 = false;
//!   let mut inserted_4 = false;
//!   let mut modified_2 = false;
//!
//!   for change in map_diff.iter_changes() {
//!     match change {
//!       MapDiff::Deleted(key) => {
//!         deleted_1 = key.reflect_partial_eq(&1).unwrap();
//!       }
//!       MapDiff::Inserted(key, value) => {
//!         inserted_4 = key.reflect_partial_eq(&4).unwrap() && value.reflect_partial_eq(&444).unwrap();
//!       }
//!       MapDiff::Modified(key, value_diff) => {
//!         modified_2 = key.reflect_partial_eq(&2).unwrap() && matches!(value_diff, Diff::Modified(..));
//!       }
//!     }
//!   }
//!
//!   assert!(deleted_1);
//!   assert!(inserted_4);
//!   assert!(modified_2);
//! }
//! ```
//!
//! [`Reflect`]: crate::Reflect
//! [`Reflect::diff`]: crate::Reflect::diff
//! [`Diff`]: crate::diff::Diff
//! [`DiffType`]: crate::diff::DiffType
//! [modified]: Diff::Modified
//! [replaced]: Diff::Replaced
//! [no change]: Diff::NoChange
//! [Myers Diffing Algorithm]: http://www.xmailserver.org/diff2.pdf

mod array_diff;
mod diff;
mod enum_diff;
mod error;
mod list_diff;
mod map_diff;
mod struct_diff;
mod tuple_diff;
mod tuple_struct_diff;
mod value_diff;

pub use array_diff::*;
pub use diff::*;
pub use enum_diff::*;
pub use error::*;
pub use list_diff::*;
pub use map_diff::*;
pub use struct_diff::*;
pub use tuple_diff::*;
pub use tuple_struct_diff::*;
pub use value_diff::*;

#[cfg(test)]
mod tests {
    use crate as bevy_reflect;
    use crate::diff::{Diff, DiffType, EnumDiff, ListDiff, MapDiff};
    use crate::Reflect;
    use bevy_utils::HashMap;

    /// Asserts that a given [`Diff`] is the correct variant and can be successfully applied.
    macro_rules! assert_diff {
        ($diff: ident, $old: ident, $new: ident, $pat: pat) => {{
            assert!(matches!($diff, $pat));

            let msg = match &$diff {
                $crate::diff::Diff::NoChange => {
                    "applying `Diff::NoChange` should result in no change"
                }
                $crate::diff::Diff::Replaced(..) => {
                    "applying `Diff::Replaced` should result in a new value"
                }
                $crate::diff::Diff::Modified(..) => {
                    "applying `Diff::Modified` should result in a modified value"
                }
            };

            // Note: We can't `take` here since `Diff::Replaced` typically returns a Dynamic
            let output = $diff.apply($old).unwrap();
            assert!(
                output.reflect_partial_eq($new).unwrap_or_default(),
                "{}",
                msg
            );
        }};
    }

    /// Diffs two values and runs the given test with the generated [`Diff`].
    ///
    /// This will run the test twice:
    /// * Once where `old` is of type `T1`
    /// * Once where `old` is a Dynamic representation of `T1` (generated via [`Reflect::clone_value`])
    fn run_diff_test<T1, T2, F>(old: T1, new: T2, test: F)
    where
        T1: Reflect + Clone,
        T2: Reflect + Clone,
        F: Fn(Diff, Box<dyn Reflect>, &dyn Reflect),
    {
        let diff = old.diff(&new).unwrap();
        test(diff, Box::new(old.clone()), &new);

        let diff = old.diff(&new).unwrap();
        test(diff, old.clone_value(), &new);
    }

    #[test]
    fn should_diff_value() {
        run_diff_test(123_i32, 123_i32, |diff, old, new| {
            assert_diff!(diff, old, new, Diff::NoChange);
        });

        run_diff_test(123_i32, 123_u32, |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Replaced(..));
        });

        run_diff_test(123_i32, 321_i32, |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Modified(..));
        });
    }

    #[test]
    fn should_diff_tuple() {
        run_diff_test((1, 2, 3), (1, 2, 3), |diff, old, new| {
            assert_diff!(diff, old, new, Diff::NoChange);
        });

        run_diff_test((1, 2, 3), (1, 2, 3, 4), |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Replaced(..));
        });

        run_diff_test((1, 2, 3), (1, 0, 3), |diff, old, new| {
            if let Diff::Modified(modified) = &diff {
                if let DiffType::Tuple(tuple_diff) = modified {
                    let mut fields = tuple_diff.field_iter();

                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(
                        fields.next(),
                        Some(Diff::Modified(DiffType::Value(..)))
                    ));
                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(fields.next(), None));
                } else {
                    panic!("expected `DiffType::Tuple`");
                }
            } else {
                panic!("expected `Diff::Modified`");
            }
            assert_diff!(diff, old, new, Diff::Modified(..));
        });
    }

    #[test]
    fn should_diff_array() {
        run_diff_test([1, 2, 3], [1, 2, 3], |diff, old, new| {
            assert_diff!(diff, old, new, Diff::NoChange);
        });

        run_diff_test([1, 2, 3], [1, 2, 3, 4], |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Replaced(..));
        });

        run_diff_test([1, 2, 3], [1, 0, 3], |diff, old, new| {
            if let Diff::Modified(modified) = &diff {
                if let DiffType::Array(array_diff) = modified {
                    let mut fields = array_diff.iter();

                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(
                        fields.next(),
                        Some(Diff::Modified(DiffType::Value(..)))
                    ));
                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(fields.next(), None));
                } else {
                    panic!("expected `DiffType::Array`");
                }
            } else {
                panic!("expected `Diff::Modified`");
            }
            assert_diff!(diff, old, new, Diff::Modified(..));
        });
    }

    #[test]
    fn should_diff_list() {
        run_diff_test(vec![1, 2, 3], vec![1, 2, 3], |diff, old, new| {
            assert_diff!(diff, old, new, Diff::NoChange);
        });

        run_diff_test(vec![1_i32, 2, 3], vec![1_u32, 2, 3], |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Replaced(..));
        });

        run_diff_test(vec![1, 2, 3], vec![9, 1, 2, 3], |diff, old, new| {
            if let Diff::Modified(modified) = &diff {
                if let DiffType::List(list_diff) = modified {
                    let mut changes = list_diff.iter_changes();

                    assert!(matches!(
                        changes.next(),
                        Some(ListDiff::Inserted(0, _ /* 9 */))
                    ));
                    assert!(matches!(changes.next(), None));
                } else {
                    panic!("expected `DiffType::List`");
                }
            } else {
                panic!("expected `Diff::Modified`");
            }
            assert_diff!(diff, old, new, Diff::Modified(..));
        });

        run_diff_test(Vec::<i32>::new(), vec![1, 2, 3], |diff, old, new| {
            if let Diff::Modified(modified) = &diff {
                if let DiffType::List(list_diff) = modified {
                    let mut changes = list_diff.iter_changes();

                    assert!(matches!(
                        changes.next(),
                        Some(ListDiff::Inserted(0, _ /* 1 */))
                    ));
                    assert!(matches!(
                        changes.next(),
                        Some(ListDiff::Inserted(0, _ /* 2 */))
                    ));
                    assert!(matches!(
                        changes.next(),
                        Some(ListDiff::Inserted(0, _ /* 3 */))
                    ));
                    assert!(matches!(changes.next(), None));
                } else {
                    panic!("expected `DiffType::List`");
                }
            } else {
                panic!("expected `Diff::Modified`");
            }
            assert_diff!(diff, old, new, Diff::Modified(..));
        });

        run_diff_test(
            vec![1, 2, 3, 4, 5],
            vec![1, 0, 3, 6, 8, 4, 7],
            |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::List(list_diff) = modified {
                        let mut changes = list_diff.iter_changes();

                        assert!(matches!(changes.next(), Some(ListDiff::Deleted(1 /* 2 */))));
                        assert!(matches!(
                            changes.next(),
                            Some(ListDiff::Inserted(2, _ /* 0 */))
                        ));
                        assert!(matches!(
                            changes.next(),
                            Some(ListDiff::Inserted(3, _ /* 6 */))
                        ));
                        assert!(matches!(
                            changes.next(),
                            Some(ListDiff::Inserted(3, _ /* 8 */))
                        ));
                        assert!(matches!(changes.next(), Some(ListDiff::Deleted(4 /* 5 */))));
                        assert!(matches!(
                            changes.next(),
                            Some(ListDiff::Inserted(5, _ /* 7 */))
                        ));
                        assert!(matches!(changes.next(), None));
                    } else {
                        panic!("expected `DiffType::List`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            },
        );
    }

    #[test]
    fn should_diff_map() {
        macro_rules! map {
            ($($key: tt : $value: expr),* $(,)?) => {
                HashMap::from([$((($key, $value))),*])
            };
        }

        run_diff_test(
            map! {1: 111, 2: 222, 3: 333},
            map! {3: 333, 1: 111, 2: 222},
            |diff, old, new| {
                assert_diff!(diff, old, new, Diff::NoChange);
            },
        );

        run_diff_test(
            map! {1: 111_i32, 2: 222, 3: 333},
            map! {3: 333_u32, 1: 111, 2: 222},
            |diff, old, new| {
                assert_diff!(diff, old, new, Diff::Replaced(..));
            },
        );

        run_diff_test(
            map! {1: 111, 2: 222, 3: 333},
            map! {3: 333, 1: 111},
            |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::Map(map_diff) = modified {
                        let mut changes = map_diff.iter_changes();

                        assert!(matches!(changes.next(), Some(MapDiff::Deleted(_ /* 2 */))));
                        assert!(matches!(changes.next(), None));
                    } else {
                        panic!("expected `DiffType::Map`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            },
        );

        run_diff_test(
            map! {1: 111, 2: 222, 3: 333},
            map! {3: 333, 1: 111, 4: 444, 2: 222},
            |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::Map(map_diff) = modified {
                        let mut changes = map_diff.iter_changes();

                        assert!(matches!(
                            changes.next(),
                            Some(MapDiff::Inserted(_ /* 4 */, _ /* 444 */))
                        ));
                        assert!(matches!(changes.next(), None));
                    } else {
                        panic!("expected `DiffType::Map`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            },
        );

        run_diff_test(
            map! {1: 111, 2: 222, 3: 333},
            map! {3: 333, 1: 111, 2: 999},
            |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::Map(map_diff) = modified {
                        let mut changes = map_diff.iter_changes();

                        assert!(matches!(
                            changes.next(),
                            Some(MapDiff::Modified(_ /* 2 */, _ /* 999 */))
                        ));
                        assert!(matches!(changes.next(), None));
                    } else {
                        panic!("expected `DiffType::Map`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            },
        );
    }

    #[test]
    fn should_diff_tuple_struct() {
        #[derive(Reflect, Clone)]
        struct Foo(i32, i32, i32);
        #[derive(Reflect, Clone)]
        struct Bar(i32, i32, i32, i32);

        run_diff_test(Foo(1, 2, 3), Foo(1, 2, 3), |diff, old, new| {
            assert_diff!(diff, old, new, Diff::NoChange);
        });

        run_diff_test(Foo(1, 2, 3), Bar(1, 2, 3, 4), |diff, old, new| {
            assert_diff!(diff, old, new, Diff::Replaced(..));
        });

        run_diff_test(Foo(1, 2, 3), Foo(1, 0, 3), |diff, old, new| {
            if let Diff::Modified(modified) = &diff {
                if let DiffType::TupleStruct(tuple_struct_diff) = modified {
                    let mut fields = tuple_struct_diff.field_iter();

                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(
                        fields.next(),
                        Some(Diff::Modified(DiffType::Value(..)))
                    ));
                    assert!(matches!(fields.next(), Some(Diff::NoChange)));
                    assert!(matches!(fields.next(), None));
                } else {
                    panic!("expected `DiffType::TupleStruct`");
                }
            } else {
                panic!("expected `Diff::Modified`");
            }
            assert_diff!(diff, old, new, Diff::Modified(..));
        });
    }

    #[test]
    fn should_diff_struct() {
        #[derive(Reflect, Clone)]
        struct Foo {
            a: i32,
            b: f32,
        }
        #[derive(Reflect, Clone)]
        struct Bar {
            a: i32,
            b: f32,
            c: usize,
        }

        run_diff_test(
            Foo { a: 123, b: 1.23 },
            Foo { a: 123, b: 1.23 },
            |diff, old, new| {
                assert_diff!(diff, old, new, Diff::NoChange);
            },
        );
        run_diff_test(
            Foo { a: 123, b: 1.23 },
            Bar {
                a: 123,
                b: 1.23,
                c: 123,
            },
            |diff, old, new| {
                assert_diff!(diff, old, new, Diff::Replaced(..));
            },
        );
        run_diff_test(
            Foo { a: 123, b: 1.23 },
            Foo { a: 123, b: 3.21 },
            |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::Struct(struct_diff) = modified {
                        let mut fields = struct_diff.field_iter();

                        assert!(matches!(fields.next(), Some(("a", Diff::NoChange))));
                        assert!(matches!(
                            fields.next(),
                            Some(("b", Diff::Modified(DiffType::Value(..))))
                        ));
                        assert!(matches!(fields.next(), None));
                    } else {
                        panic!("expected `DiffType::Struct`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            },
        );
    }

    mod enums {
        use super::*;

        #[test]
        fn should_diff_unit_variant() {
            #[derive(Reflect, Clone)]
            enum Foo {
                A,
                B,
            }
            #[derive(Reflect, Clone)]
            enum Bar {
                A,
                B,
            }

            run_diff_test(Foo::A, Foo::A, |diff, old, new| {
                assert_diff!(diff, old, new, Diff::NoChange);
            });

            run_diff_test(Foo::A, Foo::B, |diff, old, new| {
                assert!(matches!(
                    diff,
                    Diff::Modified(DiffType::Enum(EnumDiff::Swapped(..)))
                ));
                assert_diff!(diff, old, new, Diff::Modified(..));
            });

            run_diff_test(Foo::A, Bar::A, |diff, old, new| {
                assert_diff!(diff, old, new, Diff::Replaced(..));
            });
        }

        #[test]
        fn should_diff_tuple_variant() {
            #[derive(Reflect, Clone)]
            enum Foo {
                A(i32, i32, i32),
                B(i32, i32, i32),
            }
            #[derive(Reflect, Clone)]
            enum Bar {
                A(i32, i32, i32),
                B(i32, i32, i32),
            }

            run_diff_test(Foo::A(1, 2, 3), Foo::A(1, 2, 3), |diff, old, new| {
                assert_diff!(diff, old, new, Diff::NoChange);
            });

            run_diff_test(Foo::A(1, 2, 3), Foo::B(1, 2, 3), |diff, old, new| {
                assert!(matches!(
                    diff,
                    Diff::Modified(DiffType::Enum(EnumDiff::Swapped(..)))
                ));
                assert_diff!(diff, old, new, Diff::Modified(..));
            });

            run_diff_test(Foo::A(1, 2, 3), Bar::A(1, 2, 3), |diff, old, new| {
                assert_diff!(diff, old, new, Diff::Replaced(..));
            });

            run_diff_test(Foo::A(1, 2, 3), Foo::A(1, 0, 3), |diff, old, new| {
                if let Diff::Modified(modified) = &diff {
                    if let DiffType::Enum(enum_diff) = modified {
                        if let EnumDiff::Tuple(tuple_diff) = enum_diff {
                            let mut fields = tuple_diff.field_iter();

                            assert!(matches!(fields.next(), Some(Diff::NoChange)));
                            assert!(matches!(
                                fields.next(),
                                Some(Diff::Modified(DiffType::Value(..)))
                            ));
                            assert!(matches!(fields.next(), Some(Diff::NoChange)));
                            assert!(matches!(fields.next(), None));
                        } else {
                            panic!("expected `EnumDiff::Tuple`");
                        }
                    } else {
                        panic!("expected `DiffType::Enum`");
                    }
                } else {
                    panic!("expected `Diff::Modified`");
                }
                assert_diff!(diff, old, new, Diff::Modified(..));
            });
        }

        #[test]
        fn should_diff_struct_variant() {
            #[derive(Reflect, Clone)]
            enum Foo {
                A { x: f32, y: f32 },
                B { x: f32, y: f32 },
            }
            #[derive(Reflect, Clone)]
            enum Bar {
                A { x: f32, y: f32 },
                B { x: f32, y: f32 },
            }

            run_diff_test(
                Foo::A { x: 1.23, y: 4.56 },
                Foo::A { x: 1.23, y: 4.56 },
                |diff, old, new| {
                    assert_diff!(diff, old, new, Diff::NoChange);
                },
            );
            run_diff_test(
                Foo::A { x: 1.23, y: 4.56 },
                Foo::B { x: 1.23, y: 4.56 },
                |diff, old, new| {
                    assert!(matches!(
                        diff,
                        Diff::Modified(DiffType::Enum(EnumDiff::Swapped(..)))
                    ));
                    assert_diff!(diff, old, new, Diff::Modified(..));
                },
            );
            run_diff_test(
                Foo::A { x: 1.23, y: 4.56 },
                Bar::A { x: 1.23, y: 4.56 },
                |diff, old, new| {
                    assert_diff!(diff, old, new, Diff::Replaced(..));
                },
            );
            run_diff_test(
                Foo::A { x: 1.23, y: 4.56 },
                Foo::A { x: 1.23, y: 7.89 },
                |diff, old, new| {
                    if let Diff::Modified(modified) = &diff {
                        if let DiffType::Enum(enum_diff) = modified {
                            if let EnumDiff::Struct(struct_diff) = enum_diff {
                                let mut fields = struct_diff.field_iter();

                                assert!(matches!(fields.next(), Some(("x", Diff::NoChange))));
                                assert!(matches!(
                                    fields.next(),
                                    Some(("y", Diff::Modified(DiffType::Value(..))))
                                ));
                                assert!(matches!(fields.next(), None));
                            } else {
                                panic!("expected `EnumDiff::Struct`");
                            }
                        } else {
                            panic!("expected `DiffType::Enum`");
                        }
                    } else {
                        panic!("expected `Diff::Modified`");
                    }
                    assert_diff!(diff, old, new, Diff::Modified(..));
                },
            );
        }
    }
}
