use crate::utility::reflect_hasher;
use crate::{
    self as bevy_reflect, utility::GenericTypePathCell, FromReflect, GetTypeRegistration, Reflect,
    ReflectMut, ReflectOwned, ReflectRef, TypeInfo, TypePath, TypeRegistration, Typed,
    UnnamedField,
};
use bevy_reflect_derive::impl_type_path;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::slice::Iter;

/// A trait used to power [tuple-like] operations via [reflection].
///
/// This trait uses the [`Reflect`] trait to allow implementors to have their fields
/// be dynamically addressed by index.
///
/// This trait is automatically implemented for arbitrary tuples of up to 12
/// elements, provided that each element implements [`Reflect`].
///
/// # Example
///
/// ```
/// use bevy_reflect::{Reflect, Tuple};
///
/// let foo = (123_u32, true);
/// assert_eq!(foo.field_len(), 2);
///
/// let field: &dyn Reflect = foo.field(0).unwrap();
/// assert_eq!(field.downcast_ref::<u32>(), Some(&123));
/// ```
///
/// [tuple-like]: https://doc.rust-lang.org/book/ch03-02-data-types.html#the-tuple-type
/// [reflection]: crate
pub trait Tuple: Reflect {
    /// Returns a reference to the value of the field with index `index` as a
    /// `&dyn Reflect`.
    fn field(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field with index `index`
    /// as a `&mut dyn Reflect`.
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Returns the number of fields in the tuple.
    fn field_len(&self) -> usize;

    /// Returns an iterator over the values of the tuple's fields.
    fn iter_fields(&self) -> TupleFieldIter;

    /// Drain the fields of this tuple to get a vector of owned values.
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>>;

    /// Clones the struct into a [`DynamicTuple`].
    fn clone_dynamic(&self) -> DynamicTuple;
}

/// An iterator over the field values of a tuple.
pub struct TupleFieldIter<'a> {
    pub(crate) tuple: &'a dyn Tuple,
    pub(crate) index: usize,
}

impl<'a> TupleFieldIter<'a> {
    pub fn new(value: &'a dyn Tuple) -> Self {
        TupleFieldIter {
            tuple: value,
            index: 0,
        }
    }
}

impl<'a> Iterator for TupleFieldIter<'a> {
    type Item = &'a dyn Reflect;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.tuple.field(self.index);
        self.index += 1;
        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.tuple.field_len();
        (size, Some(size))
    }
}

impl<'a> ExactSizeIterator for TupleFieldIter<'a> {}

/// A convenience trait which combines fetching and downcasting of tuple
/// fields.
///
/// # Example
///
/// ```
/// use bevy_reflect::GetTupleField;
///
/// # fn main() {
/// let foo = ("blue".to_string(), 42_i32);
///
/// assert_eq!(foo.get_field::<String>(0), Some(&"blue".to_string()));
/// assert_eq!(foo.get_field::<i32>(1), Some(&42));
/// # }
/// ```
pub trait GetTupleField {
    /// Returns a reference to the value of the field with index `index`,
    /// downcast to `T`.
    fn get_field<T: Reflect>(&self, index: usize) -> Option<&T>;

    /// Returns a mutable reference to the value of the field with index
    /// `index`, downcast to `T`.
    fn get_field_mut<T: Reflect>(&mut self, index: usize) -> Option<&mut T>;
}

impl<S: Tuple> GetTupleField for S {
    fn get_field<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.field(index)
            .and_then(|value| value.downcast_ref::<T>())
    }

    fn get_field_mut<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_mut(index)
            .and_then(|value| value.downcast_mut::<T>())
    }
}

impl GetTupleField for dyn Tuple {
    fn get_field<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.field(index)
            .and_then(|value| value.downcast_ref::<T>())
    }

    fn get_field_mut<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_mut(index)
            .and_then(|value| value.downcast_mut::<T>())
    }
}

/// A container for compile-time tuple info.
#[derive(Clone, Debug)]
pub struct TupleInfo {
    type_name: &'static str,
    type_id: TypeId,
    fields: Box<[UnnamedField]>,
    meta: TupleMeta,
}

impl TupleInfo {
    /// Create a new [`TupleInfo`].
    ///
    /// # Arguments
    ///
    /// * `fields`: The fields of this tuple in the order they are defined
    ///
    pub fn new<T: Reflect>(fields: &[UnnamedField]) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: fields.to_vec().into_boxed_slice(),
            meta: TupleMeta::new(),
        }
    }

    /// Add metadata for this tuple.
    pub fn with_meta(self, meta: TupleMeta) -> Self {
        Self { meta, ..self }
    }

    /// Get the field at the given index.
    pub fn field_at(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    /// Iterate over the fields of this tuple.
    pub fn iter(&self) -> Iter<'_, UnnamedField> {
        self.fields.iter()
    }

    /// The total number of fields in this tuple.
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }

    /// The [type name] of the tuple.
    ///
    /// [type name]: std::any::type_name
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The [`TypeId`] of the tuple.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// The metadata of the struct.
    pub fn meta(&self) -> &TupleMeta {
        &self.meta
    }

    /// Check if the given type matches the tuple type.
    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }
}

/// Metadata for [tuples], accessed via [`TupleInfo::meta`].
///
/// [tuples]: Tuple
#[derive(Clone, Debug)]
pub struct TupleMeta {
    /// The docstring of this tuple, if any.
    #[cfg(feature = "documentation")]
    pub docs: Option<&'static str>,
}

impl TupleMeta {
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "documentation")]
            docs: None,
        }
    }
}

impl Default for TupleMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// A tuple which allows fields to be added at runtime.
#[derive(Default, Debug)]
pub struct DynamicTuple {
    name: Cow<'static, str>,
    represented_type: Option<&'static TypeInfo>,
    fields: Vec<Box<dyn Reflect>>,
}

impl DynamicTuple {
    /// Sets the [type] to be represented by this `DynamicTuple`.
    ///
    /// # Panics
    ///
    /// Panics if the given [type] is not a [`TypeInfo::Tuple`].
    ///
    /// [type]: TypeInfo
    pub fn set_represented_type(&mut self, represented_type: Option<&'static TypeInfo>) {
        if let Some(represented_type) = represented_type {
            assert!(
                matches!(represented_type, TypeInfo::Tuple(_)),
                "expected TypeInfo::Tuple but received: {:?}",
                represented_type
            );

            self.name = Cow::Borrowed(represented_type.type_name());
        }
        self.represented_type = represented_type;
    }

    /// Appends an element with value `value` to the tuple.
    pub fn insert_boxed(&mut self, value: Box<dyn Reflect>) {
        self.represented_type = None;
        self.fields.push(value);
        self.generate_name();
    }

    /// Appends a typed element with value `value` to the tuple.
    pub fn insert<T: Reflect>(&mut self, value: T) {
        self.represented_type = None;
        self.insert_boxed(Box::new(value));
        self.generate_name();
    }

    fn generate_name(&mut self) {
        let mut name = self.name.to_string();
        name.clear();
        name.push('(');
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                name.push_str(", ");
            }
            name.push_str(field.type_name());
        }
        name.push(')');
        self.name = Cow::Owned(name);
    }
}

impl Tuple for DynamicTuple {
    #[inline]
    fn field(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index).map(|field| &**field)
    }

    #[inline]
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index).map(|field| &mut **field)
    }

    #[inline]
    fn field_len(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn iter_fields(&self) -> TupleFieldIter {
        TupleFieldIter {
            tuple: self,
            index: 0,
        }
    }

    #[inline]
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.fields
    }

    #[inline]
    fn clone_dynamic(&self) -> DynamicTuple {
        DynamicTuple {
            name: self.name.clone(),
            represented_type: self.represented_type,
            fields: self
                .fields
                .iter()
                .map(|value| value.clone_value())
                .collect(),
        }
    }
}

impl Reflect for DynamicTuple {
    #[inline]
    fn type_name(&self) -> &str {
        self.represented_type
            .map(|info| info.type_name())
            .unwrap_or_else(|| &self.name)
    }

    #[inline]
    fn get_represented_type_info(&self) -> Option<&'static TypeInfo> {
        self.represented_type
    }

    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> {
        self
    }

    #[inline]
    fn as_reflect(&self) -> &dyn Reflect {
        self
    }

    #[inline]
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
        self
    }

    #[inline]
    fn clone_value(&self) -> Box<dyn Reflect> {
        Box::new(self.clone_dynamic())
    }

    #[inline]
    fn reflect_ref(&self) -> ReflectRef {
        ReflectRef::Tuple(self)
    }

    #[inline]
    fn reflect_mut(&mut self) -> ReflectMut {
        ReflectMut::Tuple(self)
    }

    #[inline]
    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::Tuple(self)
    }

    fn apply(&mut self, value: &dyn Reflect) {
        tuple_apply(self, value);
    }

    fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        *self = value.take()?;
        Ok(())
    }

    fn reflect_hash(&self) -> Option<u64> {
        tuple_hash(self)
    }

    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        tuple_partial_eq(self, value)
    }

    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicTuple(")?;
        tuple_debug(self, f)?;
        write!(f, ")")
    }

    #[inline]
    fn is_dynamic(&self) -> bool {
        true
    }
}

impl_type_path!((in bevy_reflect) DynamicTuple);

/// Returns the `u64` hash of the given [tuple](Tuple).
#[inline]
pub fn tuple_hash<T: Tuple>(value: &T) -> Option<u64> {
    let mut hasher = reflect_hasher();

    match value.get_represented_type_info() {
        // Proxy case
        Some(info) => {
            let TypeInfo::Tuple(info) = info else {
                return None;
            };

            Hash::hash(&info.type_id(), &mut hasher);
            Hash::hash(&value.field_len(), &mut hasher);

            for field in info.iter() {
                if let Some(value) = value.field(field.index()) {
                    Hash::hash(&value.reflect_hash()?, &mut hasher);
                }
            }
        }
        // Dynamic case
        None => {
            Hash::hash(&TypeId::of::<T>(), &mut hasher);
            Hash::hash(&value.field_len(), &mut hasher);

            for field in value.iter_fields() {
                Hash::hash(&field.reflect_hash()?, &mut hasher);
            }
        }
    }

    Some(hasher.finish())
}

/// Applies the elements of `b` to the corresponding elements of `a`.
///
/// # Panics
///
/// This function panics if `b` is not a tuple.
#[inline]
pub fn tuple_apply<T: Tuple>(a: &mut T, b: &dyn Reflect) {
    if let ReflectRef::Tuple(tuple) = b.reflect_ref() {
        for (i, value) in tuple.iter_fields().enumerate() {
            if let Some(v) = a.field_mut(i) {
                v.apply(value);
            }
        }
    } else {
        panic!("Attempted to apply non-Tuple type to Tuple type.");
    }
}

/// Compares a [`Tuple`] with a [`Reflect`] value.
///
/// Returns true if and only if all of the following are true:
/// - `b` is a tuple;
/// - `b` has the same number of elements as `a`;
/// - [`Reflect::reflect_partial_eq`] returns `Some(true)` for pairwise elements of `a` and `b`.
///
/// Returns [`None`] if the comparison couldn't even be performed.
#[inline]
pub fn tuple_partial_eq<T: Tuple>(a: &T, b: &dyn Reflect) -> Option<bool> {
    let ReflectRef::Tuple(b) = b.reflect_ref()  else {
        return Some(false);
    };

    if a.field_len() != b.field_len() {
        return Some(false);
    }

    for (value_a, value_b) in a.iter_fields().zip(b.iter_fields()) {
        if !value_a.reflect_partial_eq(value_b)? {
            return Some(false);
        }
    }

    Some(true)
}

/// The default debug formatter for [`Tuple`] types.
///
/// # Example
/// ```
/// use bevy_reflect::Reflect;
///
/// let my_tuple: &dyn Reflect = &(1, 2, 3);
/// println!("{:#?}", my_tuple);
///
/// // Output:
///
/// // (
/// //   1,
/// //   2,
/// //   3,
/// // )
/// ```
#[inline]
pub fn tuple_debug(dyn_tuple: &dyn Tuple, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_tuple("");
    for field in dyn_tuple.iter_fields() {
        debug.field(&field as &dyn Debug);
    }
    debug.finish()
}

macro_rules! impl_reflect_tuple {
    {$($index:tt : $name:tt),*} => {
        impl<$($name: Reflect + TypePath),*> Tuple for ($($name,)*) {
            #[inline]
            fn field(&self, index: usize) -> Option<&dyn Reflect> {
                match index {
                    $($index => Some(&self.$index as &dyn Reflect),)*
                    _ => None,
                }
            }

            #[inline]
            fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
                match index {
                    $($index => Some(&mut self.$index as &mut dyn Reflect),)*
                    _ => None,
                }
            }

            #[inline]
            fn field_len(&self) -> usize {
                const INDICES: &[usize] = &[$($index as usize),*];
                INDICES.len()
            }

            #[inline]
            fn iter_fields(&self) -> TupleFieldIter {
                TupleFieldIter {
                    tuple: self,
                    index: 0,
                }
            }

            #[inline]
            fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
                vec![
                    $(Box::new(self.$index),)*
                ]
            }

            #[inline]
            fn clone_dynamic(&self) -> DynamicTuple {
                let info = self.get_represented_type_info();
                DynamicTuple {
                    name: Cow::Borrowed(::core::any::type_name::<Self>()),
                    represented_type: info,
                    fields: self
                        .iter_fields()
                        .map(|value| value.clone_value())
                        .collect(),
                }
            }
        }

        impl<$($name: Reflect + TypePath),*> Reflect for ($($name,)*) {
            fn type_name(&self) -> &str {
                std::any::type_name::<Self>()
            }

            fn get_represented_type_info(&self) -> Option<&'static TypeInfo> {
                Some(<Self as Typed>::type_info())
            }

            fn into_any(self: Box<Self>) -> Box<dyn Any> {
                self
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }

            fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> {
                self
            }

            fn as_reflect(&self) -> &dyn Reflect {
                self
            }

            fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
                self
            }

            fn apply(&mut self, value: &dyn Reflect) {
                crate::tuple_apply(self, value);
            }

            fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
                *self = value.take()?;
                Ok(())
            }

            fn reflect_ref(&self) -> ReflectRef {
                ReflectRef::Tuple(self)
            }

            fn reflect_mut(&mut self) -> ReflectMut {
                ReflectMut::Tuple(self)
            }

            fn reflect_owned(self: Box<Self>) -> ReflectOwned {
                ReflectOwned::Tuple(self)
            }

            fn clone_value(&self) -> Box<dyn Reflect> {
                Box::new(self.clone_dynamic())
            }

            fn reflect_hash(&self) -> Option<u64> {
                let mut hasher = crate::utility::reflect_hasher();
                Hash::hash(&TypeId::of::<Self>(), &mut hasher);
                Hash::hash(&self.field_len(), &mut hasher);

                $(
                    Hash::hash(&self.$index.reflect_hash()?, &mut hasher);
                )*

                Some(hasher.finish())
            }

            fn reflect_partial_eq(&self, other: &dyn Reflect) -> Option<bool> {
                #[allow(unused_variables)]
                if let Some(other) = other.downcast_ref::<Self>() {
                    $(
                        if !self.$index.reflect_partial_eq(&other.$index)? {
                            return Some(false);
                        }
                    )*
                } else {
                    let ReflectRef::Tuple(other) = Reflect::reflect_ref(other) else {
                        return Some(false);
                    };

                    if other.field_len() != self.field_len() {
                        return Some(false);
                    }

                    $(
                        let Some(other_field) = other.field($index) else {
                            return Some(false);
                        };

                        if !self.$index.reflect_partial_eq(other_field)? {
                            return Some(false);
                        }
                    )*
                }

                Some(true)
            }
        }

        impl <$($name: Reflect + TypePath),*> Typed for ($($name,)*) {
            fn type_info() -> &'static TypeInfo {
                static CELL: $crate::utility::GenericTypeInfoCell = $crate::utility::GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self, _>(|| {
                    let fields = [
                        $(UnnamedField::new::<$name>($index),)*
                    ];
                    let info = TupleInfo::new::<Self>(&fields);
                    TypeInfo::Tuple(info)
                })
            }
        }

        impl <$($name: Reflect + TypePath),*> TypePath for ($($name,)*) {
            fn type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self, _>(|| {
                    "(".to_owned() $(+ <$name as TypePath>::type_path())* + ")"
                })
            }

            fn short_type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self, _>(|| {
                    "(".to_owned() $(+ <$name as TypePath>::short_type_path())* + ")"
                })
            }
        }


        impl<$($name: Reflect + TypePath),*> GetTypeRegistration for ($($name,)*) {
            fn get_type_registration() -> TypeRegistration {
                TypeRegistration::of::<($($name,)*)>()
            }
        }

        impl<$($name: FromReflect + TypePath),*> FromReflect for ($($name,)*)
        {
            fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
                if let ReflectRef::Tuple(_ref_tuple) = reflect.reflect_ref() {
                    Some(
                        (
                            $(
                                <$name as FromReflect>::from_reflect(_ref_tuple.field($index)?)?,
                            )*
                        )
                    )
                } else {
                    None
                }
            }
        }
    }
}

impl_reflect_tuple! {}
impl_reflect_tuple! {0: A}
impl_reflect_tuple! {0: A, 1: B}
impl_reflect_tuple! {0: A, 1: B, 2: C}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K}
impl_reflect_tuple! {0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K, 11: L}
