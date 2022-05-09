#![doc = include_str!("../README.md")]

mod array;
mod list;
mod map;
mod path;
mod reflect;
mod struct_trait;
mod tuple;
mod tuple_struct;
mod type_registry;
mod type_uuid;
mod impls {
    #[cfg(feature = "glam")]
    mod glam;
    #[cfg(feature = "smallvec")]
    mod smallvec;
    mod std;

    #[cfg(feature = "glam")]
    pub use self::glam::*;
    #[cfg(feature = "smallvec")]
    pub use self::smallvec::*;
    pub use self::std::*;
}

pub mod serde;
pub mod std_traits;

pub mod prelude {
    pub use crate::std_traits::*;
    #[doc(hidden)]
    pub use crate::{
        reflect_trait, GetField, GetTupleStructField, Reflect, ReflectDeserialize, Struct,
        TupleStruct,
    };
}

pub use array::*;
pub use impls::*;
pub use list::*;
pub use map::*;
pub use path::*;
pub use reflect::*;
pub use struct_trait::*;
pub use tuple::*;
pub use tuple_struct::*;
pub use type_registry::*;
pub use type_uuid::*;

pub use bevy_reflect_derive::*;
pub use erased_serde;

#[doc(hidden)]
pub mod __macro_exports {
    use crate::Uuid;

    /// Generates a new UUID from the given UUIDs `a` and `b`,
    /// where the bytes are generated by a bitwise `a ^ b.rotate_right(1)`.
    /// The generated UUID will be a `UUIDv4` (meaning that the bytes should be random, not e.g. derived from the system time).
    #[allow(clippy::unusual_byte_groupings)] // unusual byte grouping is meant to signal the relevant bits
    pub const fn generate_composite_uuid(a: Uuid, b: Uuid) -> Uuid {
        let mut new = [0; 16];
        let mut i = 0;
        while i < new.len() {
            // rotating ensures different uuids for A<B<C>> and B<A<C>> because: A ^ (B ^ C) = B ^ (A ^ C)
            // notice that you have to rotate the second parameter: A.rr ^ (B.rr ^ C) = B.rr ^ (A.rr ^ C)
            // Solution: A ^ (B ^ C.rr).rr != B ^ (A ^ C.rr).rr
            new[i] = a.as_bytes()[i] ^ b.as_bytes()[i].rotate_right(1);

            i += 1;
        }

        // Version: the most significant 4 bits in the 6th byte: 11110000
        new[6] = new[6] & 0b0000_1111 | 0b0100_0000; // set version to v4

        // Variant: the most significant 3 bits in the 8th byte: 11100000
        new[8] = new[8] & 0b000_11111 | 0b100_00000; // set variant to rfc4122

        Uuid::from_bytes(new)
    }
}

#[cfg(test)]
#[allow(clippy::blacklisted_name, clippy::approx_constant)]
mod tests {
    use ::serde::de::DeserializeSeed;
    use bevy_utils::HashMap;
    use ron::{
        ser::{to_string_pretty, PrettyConfig},
        Deserializer,
    };

    use super::*;
    use crate as bevy_reflect;
    use crate::serde::{ReflectDeserializer, ReflectSerializer};

    #[test]
    fn reflect_struct() {
        #[derive(Reflect)]
        struct Foo {
            a: u32,
            b: f32,
            c: Bar,
        }
        #[derive(Reflect)]
        struct Bar {
            x: u32,
        }

        let mut foo = Foo {
            a: 42,
            b: 3.14,
            c: Bar { x: 1 },
        };

        let a = *foo.get_field::<u32>("a").unwrap();
        assert_eq!(a, 42);

        *foo.get_field_mut::<u32>("a").unwrap() += 1;
        assert_eq!(foo.a, 43);

        let bar = foo.get_field::<Bar>("c").unwrap();
        assert_eq!(bar.x, 1);

        // nested retrieval
        let c = foo.field("c").unwrap();
        if let ReflectRef::Struct(value) = c.reflect_ref() {
            assert_eq!(*value.get_field::<u32>("x").unwrap(), 1);
        } else {
            panic!("Expected a struct.");
        }

        // patch Foo with a dynamic struct
        let mut dynamic_struct = DynamicStruct::default();
        dynamic_struct.insert("a", 123u32);
        dynamic_struct.insert("should_be_ignored", 456);

        foo.apply(&dynamic_struct);
        assert_eq!(foo.a, 123);
    }

    #[test]
    fn reflect_map() {
        #[derive(Reflect, Hash)]
        #[reflect(Hash)]
        struct Foo {
            a: u32,
            b: String,
        }

        let key_a = Foo {
            a: 1,
            b: "k1".to_string(),
        };

        let key_b = Foo {
            a: 1,
            b: "k1".to_string(),
        };

        let key_c = Foo {
            a: 3,
            b: "k3".to_string(),
        };

        let mut map = DynamicMap::default();
        map.insert(key_a, 10u32);
        assert_eq!(10, *map.get(&key_b).unwrap().downcast_ref::<u32>().unwrap());
        assert!(map.get(&key_c).is_none());
        *map.get_mut(&key_b).unwrap().downcast_mut::<u32>().unwrap() = 20;
        assert_eq!(20, *map.get(&key_b).unwrap().downcast_ref::<u32>().unwrap());
    }

    #[test]
    #[allow(clippy::blacklisted_name)]
    fn reflect_unit_struct() {
        #[derive(Reflect)]
        struct Foo(u32, u64);

        let mut foo = Foo(1, 2);
        assert_eq!(1, *foo.get_field::<u32>(0).unwrap());
        assert_eq!(2, *foo.get_field::<u64>(1).unwrap());

        let mut patch = DynamicTupleStruct::default();
        patch.insert(3u32);
        patch.insert(4u64);
        assert_eq!(3, *patch.field(0).unwrap().downcast_ref::<u32>().unwrap());
        assert_eq!(4, *patch.field(1).unwrap().downcast_ref::<u64>().unwrap());

        foo.apply(&patch);
        assert_eq!(3, foo.0);
        assert_eq!(4, foo.1);

        let mut iter = patch.iter_fields();
        assert_eq!(3, *iter.next().unwrap().downcast_ref::<u32>().unwrap());
        assert_eq!(4, *iter.next().unwrap().downcast_ref::<u64>().unwrap());
    }

    #[test]
    #[should_panic(expected = "the given key does not support hashing")]
    fn reflect_map_no_hash() {
        #[derive(Reflect)]
        struct Foo {
            a: u32,
        }

        let foo = Foo { a: 1 };

        let mut map = DynamicMap::default();
        map.insert(foo, 10u32);
    }

    #[test]
    fn reflect_ignore() {
        #[derive(Reflect)]
        struct Foo {
            a: u32,
            #[reflect(ignore)]
            _b: u32,
        }

        let foo = Foo { a: 1, _b: 2 };

        let values: Vec<u32> = foo
            .iter_fields()
            .map(|value| *value.downcast_ref::<u32>().unwrap())
            .collect();
        assert_eq!(values, vec![1]);
    }

    #[test]
    fn reflect_complex_patch() {
        #[derive(Reflect, Eq, PartialEq, Debug, FromReflect)]
        #[reflect(PartialEq)]
        struct Foo {
            a: u32,
            #[reflect(ignore)]
            _b: u32,
            c: Vec<isize>,
            d: HashMap<usize, i8>,
            e: Bar,
            f: (i32, Vec<isize>, Bar),
            g: Vec<(Baz, HashMap<usize, Bar>)>,
            h: [u32; 2],
        }

        #[derive(Reflect, Eq, PartialEq, Clone, Debug, FromReflect)]
        #[reflect(PartialEq)]
        struct Bar {
            x: u32,
        }

        #[derive(Reflect, Eq, PartialEq, Debug, FromReflect)]
        struct Baz(String);

        let mut hash_map = HashMap::default();
        hash_map.insert(1, 1);
        hash_map.insert(2, 2);

        let mut hash_map_baz = HashMap::default();
        hash_map_baz.insert(1, Bar { x: 0 });

        let mut foo = Foo {
            a: 1,
            _b: 1,
            c: vec![1, 2],
            d: hash_map,
            e: Bar { x: 1 },
            f: (1, vec![1, 2], Bar { x: 1 }),
            g: vec![(Baz("string".to_string()), hash_map_baz)],
            h: [2; 2],
        };

        let mut foo_patch = DynamicStruct::default();
        foo_patch.insert("a", 2u32);
        foo_patch.insert("b", 2u32); // this should be ignored

        let mut list = DynamicList::default();
        list.push(3isize);
        list.push(4isize);
        list.push(5isize);
        foo_patch.insert("c", List::clone_dynamic(&list));

        let mut map = DynamicMap::default();
        map.insert(2usize, 3i8);
        foo_patch.insert("d", map);

        let mut bar_patch = DynamicStruct::default();
        bar_patch.insert("x", 2u32);
        foo_patch.insert("e", bar_patch.clone_dynamic());

        let mut tuple = DynamicTuple::default();
        tuple.insert(2i32);
        tuple.insert(list);
        tuple.insert(bar_patch);
        foo_patch.insert("f", tuple);

        let mut composite = DynamicList::default();
        composite.push({
            let mut tuple = DynamicTuple::default();
            tuple.insert({
                let mut tuple_struct = DynamicTupleStruct::default();
                tuple_struct.insert("new_string".to_string());
                tuple_struct
            });
            tuple.insert({
                let mut map = DynamicMap::default();
                map.insert(1usize, {
                    let mut struct_ = DynamicStruct::default();
                    struct_.insert("x", 7u32);
                    struct_
                });
                map
            });
            tuple
        });
        foo_patch.insert("g", composite);

        let array = DynamicArray::from_vec(vec![2u32, 2u32]);
        foo_patch.insert("h", array);

        foo.apply(&foo_patch);

        let mut hash_map = HashMap::default();
        hash_map.insert(1, 1);
        hash_map.insert(2, 3);

        let mut hash_map_baz = HashMap::default();
        hash_map_baz.insert(1, Bar { x: 7 });

        let expected_foo = Foo {
            a: 2,
            _b: 1,
            c: vec![3, 4, 5],
            d: hash_map,
            e: Bar { x: 2 },
            f: (2, vec![3, 4, 5], Bar { x: 2 }),
            g: vec![(Baz("new_string".to_string()), hash_map_baz.clone())],
            h: [2; 2],
        };

        assert_eq!(foo, expected_foo);

        let new_foo = Foo::from_reflect(&foo_patch)
            .expect("error while creating a concrete type from a dynamic type");

        let mut hash_map = HashMap::default();
        hash_map.insert(2, 3);

        let expected_new_foo = Foo {
            a: 2,
            _b: 0,
            c: vec![3, 4, 5],
            d: hash_map,
            e: Bar { x: 2 },
            f: (2, vec![3, 4, 5], Bar { x: 2 }),
            g: vec![(Baz("new_string".to_string()), hash_map_baz)],
            h: [2; 2],
        };

        assert_eq!(new_foo, expected_new_foo);
    }

    #[test]
    fn reflect_serialize() {
        #[derive(Reflect)]
        struct Foo {
            a: u32,
            #[reflect(ignore)]
            _b: u32,
            c: Vec<isize>,
            d: HashMap<usize, i8>,
            e: Bar,
            f: String,
            g: (i32, Vec<isize>, Bar),
            h: [u32; 2],
        }

        #[derive(Reflect)]
        struct Bar {
            x: u32,
        }

        let mut hash_map = HashMap::default();
        hash_map.insert(1, 1);
        hash_map.insert(2, 2);
        let foo = Foo {
            a: 1,
            _b: 1,
            c: vec![1, 2],
            d: hash_map,
            e: Bar { x: 1 },
            f: "hi".to_string(),
            g: (1, vec![1, 2], Bar { x: 1 }),
            h: [2; 2],
        };

        let mut registry = TypeRegistry::default();
        registry.register::<u32>();
        registry.register::<isize>();
        registry.register::<usize>();
        registry.register::<Bar>();
        registry.register::<String>();
        registry.register::<i8>();
        registry.register::<i32>();

        let serializer = ReflectSerializer::new(&foo, &registry);
        let serialized = to_string_pretty(&serializer, PrettyConfig::default()).unwrap();

        let mut deserializer = Deserializer::from_str(&serialized).unwrap();
        let reflect_deserializer = ReflectDeserializer::new(&registry);
        let value = reflect_deserializer.deserialize(&mut deserializer).unwrap();
        let dynamic_struct = value.take::<DynamicStruct>().unwrap();

        assert!(foo.reflect_partial_eq(&dynamic_struct).unwrap());
    }

    #[test]
    fn reflect_take() {
        #[derive(Reflect, Debug, PartialEq)]
        #[reflect(PartialEq)]
        struct Bar {
            x: u32,
        }

        let x: Box<dyn Reflect> = Box::new(Bar { x: 2 });
        let y = x.take::<Bar>().unwrap();
        assert_eq!(y, Bar { x: 2 });
    }

    #[test]
    fn dynamic_names() {
        let list = Vec::<usize>::new();
        let dyn_list = List::clone_dynamic(&list);
        assert_eq!(dyn_list.type_name(), std::any::type_name::<Vec<usize>>());

        let array = [b'0'; 4];
        let dyn_array = Array::clone_dynamic(&array);
        assert_eq!(dyn_array.type_name(), std::any::type_name::<[u8; 4]>());

        let map = HashMap::<usize, String>::default();
        let dyn_map = map.clone_dynamic();
        assert_eq!(
            dyn_map.type_name(),
            std::any::type_name::<HashMap<usize, String>>()
        );

        let tuple = (0usize, "1".to_string(), 2.0f32);
        let mut dyn_tuple = tuple.clone_dynamic();
        dyn_tuple.insert::<usize>(3);
        assert_eq!(
            dyn_tuple.type_name(),
            std::any::type_name::<(usize, String, f32, usize)>()
        );

        #[derive(Reflect)]
        struct TestStruct {
            a: usize,
        }
        let struct_ = TestStruct { a: 0 };
        let dyn_struct = struct_.clone_dynamic();
        assert_eq!(dyn_struct.type_name(), std::any::type_name::<TestStruct>());

        #[derive(Reflect)]
        struct TestTupleStruct(usize);
        let tuple_struct = TestTupleStruct(0);
        let dyn_tuple_struct = tuple_struct.clone_dynamic();
        assert_eq!(
            dyn_tuple_struct.type_name(),
            std::any::type_name::<TestTupleStruct>()
        );
    }

    #[test]
    fn as_reflect() {
        trait TestTrait: Reflect {}

        #[derive(Reflect)]
        struct TestStruct;

        impl TestTrait for TestStruct {}

        let trait_object: Box<dyn TestTrait> = Box::new(TestStruct);

        // Should compile:
        let _ = trait_object.as_reflect();
    }
}
