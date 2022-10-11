#![doc = include_str!("../README.md")]

mod array;
mod fields;
mod list;
mod map;
mod path;
mod reflect;
mod struct_trait;
mod tuple;
mod tuple_struct;
mod type_info;
mod type_registry;
mod type_uuid;
mod impls {
    #[cfg(feature = "glam")]
    mod glam;
    #[cfg(feature = "bevy_math")]
    mod rect;
    #[cfg(feature = "smallvec")]
    mod smallvec;
    mod std;

    #[cfg(feature = "glam")]
    pub use self::glam::*;
    #[cfg(feature = "bevy_math")]
    pub use self::rect::*;
    #[cfg(feature = "smallvec")]
    pub use self::smallvec::*;
    pub use self::std::*;
}

mod enums;
pub mod serde;
pub mod std_traits;
pub mod utility;

pub mod prelude {
    pub use crate::std_traits::*;
    #[doc(hidden)]
    pub use crate::{
        reflect_trait, FromReflect, GetField, GetTupleStructField, Reflect, ReflectDeserialize,
        ReflectSerialize, Struct, TupleStruct,
    };
}

pub use array::*;
pub use enums::*;
pub use fields::*;
pub use impls::*;
pub use list::*;
pub use map::*;
pub use path::*;
pub use reflect::*;
pub use struct_trait::*;
pub use tuple::*;
pub use tuple_struct::*;
pub use type_info::*;
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
#[allow(clippy::disallowed_types, clippy::approx_constant)]
mod tests {
    #[cfg(feature = "glam")]
    use ::glam::{vec3, Vec3};
    use ::serde::{de::DeserializeSeed, Deserialize, Serialize};
    use bevy_utils::HashMap;
    use ron::{
        ser::{to_string_pretty, PrettyConfig},
        Deserializer,
    };
    use std::fmt::{Debug, Formatter};

    use super::prelude::*;
    use super::*;
    use crate as bevy_reflect;
    use crate::serde::{ReflectSerializer, UntypedReflectDeserializer};

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
    #[allow(clippy::disallowed_types)]
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
    fn from_reflect_should_use_default_field_attributes() {
        #[derive(Reflect, FromReflect, Eq, PartialEq, Debug)]
        struct MyStruct {
            // Use `Default::default()`
            // Note that this isn't an ignored field
            #[reflect(default)]
            foo: String,

            // Use `get_bar_default()`
            #[reflect(default = "get_bar_default")]
            #[reflect(ignore)]
            bar: usize,
        }

        fn get_bar_default() -> usize {
            123
        }

        let expected = MyStruct {
            foo: String::default(),
            bar: 123,
        };

        let dyn_struct = DynamicStruct::default();
        let my_struct = <MyStruct as FromReflect>::from_reflect(&dyn_struct);

        assert_eq!(Some(expected), my_struct);
    }

    #[test]
    fn from_reflect_should_use_default_container_attribute() {
        #[derive(Reflect, FromReflect, Eq, PartialEq, Debug)]
        #[reflect(Default)]
        struct MyStruct {
            foo: String,
            #[reflect(ignore)]
            bar: usize,
        }

        impl Default for MyStruct {
            fn default() -> Self {
                Self {
                    foo: String::from("Hello"),
                    bar: 123,
                }
            }
        }

        let expected = MyStruct {
            foo: String::from("Hello"),
            bar: 123,
        };

        let dyn_struct = DynamicStruct::default();
        let my_struct = <MyStruct as FromReflect>::from_reflect(&dyn_struct);

        assert_eq!(Some(expected), my_struct);
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
        map.insert(3usize, 4i8);
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
        hash_map.insert(3, 4);

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
        hash_map.insert(3, 4);

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

        #[derive(Reflect, Serialize, Deserialize)]
        #[reflect(Serialize, Deserialize)]
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
        registry.register::<i8>();
        registry.register::<i32>();
        registry.register::<usize>();
        registry.register::<isize>();
        registry.register::<Foo>();
        registry.register::<Bar>();
        registry.register::<String>();
        registry.register::<Vec<isize>>();
        registry.register::<HashMap<usize, i8>>();
        registry.register::<(i32, Vec<isize>, Bar)>();
        registry.register::<[u32; 2]>();

        let serializer = ReflectSerializer::new(&foo, &registry);
        let serialized = to_string_pretty(&serializer, PrettyConfig::default()).unwrap();

        let mut deserializer = Deserializer::from_str(&serialized).unwrap();
        let reflect_deserializer = UntypedReflectDeserializer::new(&registry);
        let value = reflect_deserializer.deserialize(&mut deserializer).unwrap();
        let dynamic_struct = value.take::<DynamicStruct>().unwrap();

        assert!(foo.reflect_partial_eq(&dynamic_struct).unwrap());
    }

    #[test]
    fn reflect_downcast() {
        #[derive(Reflect, Clone, Debug, PartialEq)]
        struct Bar {
            y: u8,
        }

        #[derive(Reflect, Clone, Debug, PartialEq)]
        struct Foo {
            x: i32,
            s: String,
            b: Bar,
            u: usize,
            t: ([f32; 3], String),
        }

        let foo = Foo {
            x: 123,
            s: "String".to_string(),
            b: Bar { y: 255 },
            u: 1111111111111,
            t: ([3.0, 2.0, 1.0], "Tuple String".to_string()),
        };

        let foo2: Box<dyn Reflect> = Box::new(foo.clone());

        assert_eq!(foo, *foo2.downcast::<Foo>().unwrap());
    }

    #[test]
    fn should_drain_fields() {
        let array_value: Box<dyn Array> = Box::new([123_i32, 321_i32]);
        let fields = array_value.drain();
        assert!(fields[0].reflect_partial_eq(&123_i32).unwrap_or_default());
        assert!(fields[1].reflect_partial_eq(&321_i32).unwrap_or_default());

        let list_value: Box<dyn List> = Box::new(vec![123_i32, 321_i32]);
        let fields = list_value.drain();
        assert!(fields[0].reflect_partial_eq(&123_i32).unwrap_or_default());
        assert!(fields[1].reflect_partial_eq(&321_i32).unwrap_or_default());

        let tuple_value: Box<dyn Tuple> = Box::new((123_i32, 321_i32));
        let fields = tuple_value.drain();
        assert!(fields[0].reflect_partial_eq(&123_i32).unwrap_or_default());
        assert!(fields[1].reflect_partial_eq(&321_i32).unwrap_or_default());

        let map_value: Box<dyn Map> = Box::new(HashMap::from([(123_i32, 321_i32)]));
        let fields = map_value.drain();
        assert!(fields[0].0.reflect_partial_eq(&123_i32).unwrap_or_default());
        assert!(fields[0].1.reflect_partial_eq(&321_i32).unwrap_or_default());
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
    fn reflect_type_info() {
        // TypeInfo
        let info = i32::type_info();
        assert_eq!(std::any::type_name::<i32>(), info.type_name());
        assert_eq!(std::any::TypeId::of::<i32>(), info.type_id());

        // TypeInfo (unsized)
        assert_eq!(
            std::any::TypeId::of::<dyn Reflect>(),
            <dyn Reflect as Typed>::type_info().type_id()
        );

        // TypeInfo (instance)
        let value: &dyn Reflect = &123_i32;
        let info = value.get_type_info();
        assert!(info.is::<i32>());

        // Struct
        #[derive(Reflect)]
        struct MyStruct {
            foo: i32,
            bar: usize,
        }

        let info = MyStruct::type_info();
        if let TypeInfo::Struct(info) = info {
            assert!(info.is::<MyStruct>());
            assert_eq!(std::any::type_name::<MyStruct>(), info.type_name());
            assert_eq!(
                std::any::type_name::<i32>(),
                info.field("foo").unwrap().type_name()
            );
            assert_eq!(
                std::any::TypeId::of::<i32>(),
                info.field("foo").unwrap().type_id()
            );
            assert!(info.field("foo").unwrap().is::<i32>());
            assert_eq!("foo", info.field("foo").unwrap().name());
            assert_eq!(
                std::any::type_name::<usize>(),
                info.field_at(1).unwrap().type_name()
            );
        } else {
            panic!("Expected `TypeInfo::Struct`");
        }

        let value: &dyn Reflect = &MyStruct { foo: 123, bar: 321 };
        let info = value.get_type_info();
        assert!(info.is::<MyStruct>());

        // Struct (generic)
        #[derive(Reflect)]
        struct MyGenericStruct<T: Reflect> {
            foo: T,
            bar: usize,
        }

        let info = <MyGenericStruct<i32>>::type_info();
        if let TypeInfo::Struct(info) = info {
            assert!(info.is::<MyGenericStruct<i32>>());
            assert_eq!(
                std::any::type_name::<MyGenericStruct<i32>>(),
                info.type_name()
            );
            assert_eq!(
                std::any::type_name::<i32>(),
                info.field("foo").unwrap().type_name()
            );
            assert_eq!("foo", info.field("foo").unwrap().name());
            assert_eq!(
                std::any::type_name::<usize>(),
                info.field_at(1).unwrap().type_name()
            );
        } else {
            panic!("Expected `TypeInfo::Struct`");
        }

        let value: &dyn Reflect = &MyGenericStruct {
            foo: String::from("Hello!"),
            bar: 321,
        };
        let info = value.get_type_info();
        assert!(info.is::<MyGenericStruct<String>>());

        // Tuple Struct
        #[derive(Reflect)]
        struct MyTupleStruct(usize, i32, MyStruct);

        let info = MyTupleStruct::type_info();
        if let TypeInfo::TupleStruct(info) = info {
            assert!(info.is::<MyTupleStruct>());
            assert_eq!(std::any::type_name::<MyTupleStruct>(), info.type_name());
            assert_eq!(
                std::any::type_name::<i32>(),
                info.field_at(1).unwrap().type_name()
            );
            assert!(info.field_at(1).unwrap().is::<i32>());
        } else {
            panic!("Expected `TypeInfo::TupleStruct`");
        }

        // Tuple
        type MyTuple = (u32, f32, String);

        let info = MyTuple::type_info();
        if let TypeInfo::Tuple(info) = info {
            assert!(info.is::<MyTuple>());
            assert_eq!(std::any::type_name::<MyTuple>(), info.type_name());
            assert_eq!(
                std::any::type_name::<f32>(),
                info.field_at(1).unwrap().type_name()
            );
        } else {
            panic!("Expected `TypeInfo::Tuple`");
        }

        let value: &dyn Reflect = &(123_u32, 1.23_f32, String::from("Hello!"));
        let info = value.get_type_info();
        assert!(info.is::<MyTuple>());

        // List
        type MyList = Vec<usize>;

        let info = MyList::type_info();
        if let TypeInfo::List(info) = info {
            assert!(info.is::<MyList>());
            assert!(info.item_is::<usize>());
            assert_eq!(std::any::type_name::<MyList>(), info.type_name());
            assert_eq!(std::any::type_name::<usize>(), info.item_type_name());
        } else {
            panic!("Expected `TypeInfo::List`");
        }

        let value: &dyn Reflect = &vec![123_usize];
        let info = value.get_type_info();
        assert!(info.is::<MyList>());

        // List (SmallVec)
        #[cfg(feature = "smallvec")]
        {
            type MySmallVec = smallvec::SmallVec<[String; 2]>;

            let info = MySmallVec::type_info();
            if let TypeInfo::List(info) = info {
                assert!(info.is::<MySmallVec>());
                assert!(info.item_is::<String>());
                assert_eq!(std::any::type_name::<MySmallVec>(), info.type_name());
                assert_eq!(std::any::type_name::<String>(), info.item_type_name());
            } else {
                panic!("Expected `TypeInfo::List`");
            }

            let value: MySmallVec = smallvec::smallvec![String::default(); 2];
            let value: &dyn Reflect = &value;
            let info = value.get_type_info();
            assert!(info.is::<MySmallVec>());
        }

        // Array
        type MyArray = [usize; 3];

        let info = MyArray::type_info();
        if let TypeInfo::Array(info) = info {
            assert!(info.is::<MyArray>());
            assert!(info.item_is::<usize>());
            assert_eq!(std::any::type_name::<MyArray>(), info.type_name());
            assert_eq!(std::any::type_name::<usize>(), info.item_type_name());
            assert_eq!(3, info.capacity());
        } else {
            panic!("Expected `TypeInfo::Array`");
        }

        let value: &dyn Reflect = &[1usize, 2usize, 3usize];
        let info = value.get_type_info();
        assert!(info.is::<MyArray>());

        // Map
        type MyMap = HashMap<usize, f32>;

        let info = MyMap::type_info();
        if let TypeInfo::Map(info) = info {
            assert!(info.is::<MyMap>());
            assert!(info.key_is::<usize>());
            assert!(info.value_is::<f32>());
            assert_eq!(std::any::type_name::<MyMap>(), info.type_name());
            assert_eq!(std::any::type_name::<usize>(), info.key_type_name());
            assert_eq!(std::any::type_name::<f32>(), info.value_type_name());
        } else {
            panic!("Expected `TypeInfo::Map`");
        }

        let value: &dyn Reflect = &MyMap::new();
        let info = value.get_type_info();
        assert!(info.is::<MyMap>());

        // Value
        type MyValue = String;

        let info = MyValue::type_info();
        if let TypeInfo::Value(info) = info {
            assert!(info.is::<MyValue>());
            assert_eq!(std::any::type_name::<MyValue>(), info.type_name());
        } else {
            panic!("Expected `TypeInfo::Value`");
        }

        let value: &dyn Reflect = &String::from("Hello!");
        let info = value.get_type_info();
        assert!(info.is::<MyValue>());

        // Dynamic
        type MyDynamic = DynamicList;

        let info = MyDynamic::type_info();
        if let TypeInfo::Dynamic(info) = info {
            assert!(info.is::<MyDynamic>());
            assert_eq!(std::any::type_name::<MyDynamic>(), info.type_name());
        } else {
            panic!("Expected `TypeInfo::Dynamic`");
        }

        let value: &dyn Reflect = &DynamicList::default();
        let info = value.get_type_info();
        assert!(info.is::<MyDynamic>());
    }

    #[cfg(feature = "documentation")]
    mod docstrings {
        use super::*;

        #[test]
        fn should_not_contain_docs() {
            #[derive(Reflect)]
            struct SomeStruct;

            let info = <SomeStruct as Typed>::type_info();
            assert_eq!(None, info.docs());
        }

        #[test]
        fn should_contain_docs() {
            /// Some struct.
            ///
            /// # Example
            ///
            /// ```ignore
            /// let some_struct = SomeStruct;
            /// ```
            #[derive(Reflect)]
            struct SomeStruct;

            let info = <SomeStruct as Typed>::type_info();
            assert_eq!(
                Some(" Some struct.\n\n # Example\n\n ```ignore\n let some_struct = SomeStruct;\n ```"),
                info.docs()
            );

            /// Some tuple struct.
            #[derive(Reflect)]
            struct SomeTupleStruct(usize);

            let info = <SomeTupleStruct as Typed>::type_info();
            assert_eq!(Some(" Some tuple struct."), info.docs());

            /// Some enum.
            #[derive(Reflect)]
            enum SomeEnum {
                Foo,
            }

            let info = <SomeEnum as Typed>::type_info();
            assert_eq!(Some(" Some enum."), info.docs());
        }

        #[test]
        fn fields_should_contain_docs() {
            #[derive(Reflect)]
            struct SomeStruct {
                /// The name
                name: String,
                /// The index
                index: usize,
                // Not documented...
                data: Vec<i32>,
            }

            let info = <SomeStruct as Typed>::type_info();
            if let TypeInfo::Struct(info) = info {
                let mut fields = info.iter();
                assert_eq!(Some(" The name"), fields.next().unwrap().docs());
                assert_eq!(Some(" The index"), fields.next().unwrap().docs());
                assert_eq!(None, fields.next().unwrap().docs());
            } else {
                panic!("expected struct info");
            }
        }

        #[test]
        fn variants_should_contain_docs() {
            #[derive(Reflect)]
            enum SomeEnum {
                // Not documented...
                Nothing,
                /// Option A
                A(
                    /// Index
                    usize,
                ),
                /// Option B
                B {
                    /// Name
                    name: String,
                },
            }

            let info = <SomeEnum as Typed>::type_info();
            if let TypeInfo::Enum(info) = info {
                let mut variants = info.iter();
                assert_eq!(None, variants.next().unwrap().docs());

                let variant = variants.next().unwrap();
                assert_eq!(Some(" Option A"), variant.docs());
                if let VariantInfo::Tuple(variant) = variant {
                    let field = variant.field_at(0).unwrap();
                    assert_eq!(Some(" Index"), field.docs());
                } else {
                    panic!("expected tuple variant")
                }

                let variant = variants.next().unwrap();
                assert_eq!(Some(" Option B"), variant.docs());
                if let VariantInfo::Struct(variant) = variant {
                    let field = variant.field_at(0).unwrap();
                    assert_eq!(Some(" Name"), field.docs());
                } else {
                    panic!("expected struct variant")
                }
            } else {
                panic!("expected enum info");
            }
        }
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

    #[test]
    fn should_reflect_debug() {
        #[derive(Reflect)]
        struct Test {
            value: usize,
            list: Vec<String>,
            array: [f32; 3],
            map: HashMap<i32, f32>,
            a_struct: SomeStruct,
            a_tuple_struct: SomeTupleStruct,
            enum_unit: SomeEnum,
            enum_tuple: SomeEnum,
            enum_struct: SomeEnum,
            custom: CustomDebug,
            #[reflect(ignore)]
            #[allow(dead_code)]
            ignored: isize,
        }

        #[derive(Reflect)]
        struct SomeStruct {
            foo: String,
        }

        #[derive(Reflect)]
        enum SomeEnum {
            A,
            B(usize),
            C { value: i32 },
        }

        #[derive(Reflect)]
        struct SomeTupleStruct(String);

        #[derive(Reflect)]
        #[reflect(Debug)]
        struct CustomDebug;
        impl Debug for CustomDebug {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str("Cool debug!")
            }
        }

        let mut map = HashMap::new();
        map.insert(123, 1.23);

        let test = Test {
            value: 123,
            list: vec![String::from("A"), String::from("B"), String::from("C")],
            array: [1.0, 2.0, 3.0],
            map,
            a_struct: SomeStruct {
                foo: String::from("A Struct!"),
            },
            a_tuple_struct: SomeTupleStruct(String::from("A Tuple Struct!")),
            enum_unit: SomeEnum::A,
            enum_tuple: SomeEnum::B(123),
            enum_struct: SomeEnum::C { value: 321 },
            custom: CustomDebug,
            ignored: 321,
        };

        let reflected: &dyn Reflect = &test;
        let expected = r#"
bevy_reflect::tests::should_reflect_debug::Test {
    value: 123,
    list: [
        "A",
        "B",
        "C",
    ],
    array: [
        1.0,
        2.0,
        3.0,
    ],
    map: {
        123: 1.23,
    },
    a_struct: bevy_reflect::tests::should_reflect_debug::SomeStruct {
        foo: "A Struct!",
    },
    a_tuple_struct: bevy_reflect::tests::should_reflect_debug::SomeTupleStruct(
        "A Tuple Struct!",
    ),
    enum_unit: A,
    enum_tuple: B(
        123,
    ),
    enum_struct: C {
        value: 321,
    },
    custom: Cool debug!,
}"#;

        assert_eq!(expected, format!("\n{:#?}", reflected));
    }

    #[test]
    fn multiple_reflect_lists() {
        #[derive(Hash, PartialEq, Reflect)]
        #[reflect(Debug, Hash)]
        #[reflect(PartialEq)]
        struct Foo(i32);

        impl Debug for Foo {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "Foo")
            }
        }

        let foo = Foo(123);
        let foo: &dyn Reflect = &foo;

        assert!(foo.reflect_hash().is_some());
        assert_eq!(Some(true), foo.reflect_partial_eq(foo));
        assert_eq!("Foo".to_string(), format!("{foo:?}"));
    }

    #[test]
    fn multiple_reflect_value_lists() {
        #[derive(Clone, Hash, PartialEq, Reflect)]
        #[reflect_value(Debug, Hash)]
        #[reflect_value(PartialEq)]
        struct Foo(i32);

        impl Debug for Foo {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "Foo")
            }
        }

        let foo = Foo(123);
        let foo: &dyn Reflect = &foo;

        assert!(foo.reflect_hash().is_some());
        assert_eq!(Some(true), foo.reflect_partial_eq(foo));
        assert_eq!("Foo".to_string(), format!("{foo:?}"));
    }

    #[cfg(feature = "glam")]
    mod glam {
        use super::*;

        #[test]
        fn vec3_serialization() {
            let v = vec3(12.0, 3.0, -6.9);

            let mut registry = TypeRegistry::default();
            registry.register::<f32>();
            registry.register::<Vec3>();

            let ser = ReflectSerializer::new(&v, &registry);

            let config = PrettyConfig::default()
                .new_line(String::from("\n"))
                .indentor(String::from("    "));
            let output = to_string_pretty(&ser, config).unwrap();
            let expected = r#"
{
    "glam::f32::vec3::Vec3": (
        x: 12.0,
        y: 3.0,
        z: -6.9,
    ),
}"#;

            assert_eq!(expected, format!("\n{}", output));
        }

        #[test]
        fn vec3_deserialization() {
            let data = r#"
{
    "glam::f32::vec3::Vec3": (
        x: 12.0,
        y: 3.0,
        z: -6.9,
    ),
}"#;

            let mut registry = TypeRegistry::default();
            registry.add_registration(Vec3::get_type_registration());
            registry.add_registration(f32::get_type_registration());

            let de = UntypedReflectDeserializer::new(&registry);

            let mut deserializer =
                ron::de::Deserializer::from_str(data).expect("Failed to acquire deserializer");

            let dynamic_struct = de
                .deserialize(&mut deserializer)
                .expect("Failed to deserialize");

            let mut result = Vec3::default();

            result.apply(&*dynamic_struct);

            assert_eq!(result, vec3(12.0, 3.0, -6.9));
        }

        #[test]
        fn vec3_field_access() {
            let mut v = vec3(1.0, 2.0, 3.0);

            assert_eq!(*v.get_field::<f32>("x").unwrap(), 1.0);

            *v.get_field_mut::<f32>("y").unwrap() = 6.0;

            assert_eq!(v.y, 6.0);
        }

        #[test]
        fn vec3_path_access() {
            let mut v = vec3(1.0, 2.0, 3.0);

            assert_eq!(*v.path("x").unwrap().downcast_ref::<f32>().unwrap(), 1.0);

            *v.path_mut("y").unwrap().downcast_mut::<f32>().unwrap() = 6.0;

            assert_eq!(v.y, 6.0);
        }

        #[test]
        fn vec3_apply_dynamic() {
            let mut v = vec3(3.0, 3.0, 3.0);

            let mut d = DynamicStruct::default();
            d.insert("x", 4.0f32);
            d.insert("y", 2.0f32);
            d.insert("z", 1.0f32);

            v.apply(&d);

            assert_eq!(v, vec3(4.0, 2.0, 1.0));
        }
    }
}
