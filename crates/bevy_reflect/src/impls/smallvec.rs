use smallvec::SmallVec;
use std::any::Any;

use crate::diff::{
    diff_list, DiffApplyError, DiffResult, DiffedArray, DiffedList, ListDiff, ValueDiff,
};
use crate::utility::GenericTypeInfoCell;
use crate::{
    Array, ArrayIter, FromReflect, FromType, GetTypeRegistration, List, ListInfo, Reflect,
    ReflectFromPtr, ReflectMut, ReflectOwned, ReflectRef, TypeInfo, TypeRegistration, Typed,
};

impl<T: smallvec::Array + Send + Sync + 'static> Array for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        if index < SmallVec::len(self) {
            Some(&self[index] as &dyn Reflect)
        } else {
            None
        }
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        if index < SmallVec::len(self) {
            Some(&mut self[index] as &mut dyn Reflect)
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        <SmallVec<T>>::len(self)
    }

    fn iter(&self) -> ArrayIter {
        ArrayIter {
            array: self,
            index: 0,
        }
    }

    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.into_iter()
            .map(|value| Box::new(value) as Box<dyn Reflect>)
            .collect()
    }

    fn apply_array_diff(
        self: Box<Self>,
        _diff: DiffedArray,
    ) -> Result<Box<dyn Reflect>, DiffApplyError> {
        Err(DiffApplyError::ExpectedList)
    }
}

impl<T: smallvec::Array + Send + Sync + 'static> List for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn push(&mut self, value: Box<dyn Reflect>) {
        let value = value.take::<T::Item>().unwrap_or_else(|value| {
            <T as smallvec::Array>::Item::from_reflect(&*value).unwrap_or_else(|| {
                panic!(
                    "Attempted to push invalid value of type {}.",
                    value.type_name()
                )
            })
        });
        SmallVec::push(self, value);
    }

    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        self.pop().map(|value| Box::new(value) as Box<dyn Reflect>)
    }

    fn apply_list_diff(
        self: Box<Self>,
        diff: DiffedList,
    ) -> Result<Box<dyn Reflect>, DiffApplyError> {
        if self.type_name() != diff.type_name() {
            return Err(DiffApplyError::TypeMismatch);
        }

        let new_len = (self.len() + diff.total_insertions()) - diff.total_deletions();
        let mut new = SmallVec::<T>::with_capacity(new_len);

        let mut changes = diff.take_changes();
        changes.reverse();

        fn has_change(changes: &[ListDiff], index: usize) -> bool {
            changes
                .last()
                .map(|change| change.index() == index)
                .unwrap_or_default()
        }

        let insert = |value: ValueDiff, list: &mut SmallVec<T>| -> Result<(), DiffApplyError> {
            Ok(list.push(
                FromReflect::from_reflect(value.as_reflect())
                    .ok_or(DiffApplyError::TypeMismatch)?,
            ))
        };

        'outer: for (curr_index, element) in self.into_iter().enumerate() {
            while has_change(&changes, curr_index) {
                match changes.pop().unwrap() {
                    ListDiff::Deleted(_) => {
                        continue 'outer;
                    }
                    ListDiff::Inserted(_, value) => {
                        insert(value, &mut new)?;
                    }
                }
            }

            new.push(element);
        }

        // Insert any remaining elements
        while let Some(ListDiff::Inserted(_, value)) = changes.pop() {
            insert(value, &mut new)?;
        }

        Ok(Box::new(new))
    }
}

impl<T: smallvec::Array + Send + Sync + 'static> Reflect for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn type_name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn get_type_info(&self) -> &'static TypeInfo {
        <Self as Typed>::type_info()
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
        crate::list_apply(self, value);
    }

    fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        *self = value.take()?;
        Ok(())
    }

    fn reflect_ref(&self) -> ReflectRef {
        ReflectRef::List(self)
    }

    fn reflect_mut(&mut self) -> ReflectMut {
        ReflectMut::List(self)
    }

    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::List(self)
    }

    fn clone_value(&self) -> Box<dyn Reflect> {
        Box::new(List::clone_dynamic(self))
    }

    fn diff<'new>(&self, other: &'new dyn Reflect) -> DiffResult<'_, 'new> {
        diff_list(self, other)
    }

    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::list_partial_eq(self, value)
    }
}

impl<T: smallvec::Array + Send + Sync + 'static> Typed for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self, _>(|| TypeInfo::List(ListInfo::new::<Self, T::Item>()))
    }
}

impl<T: smallvec::Array + Send + Sync + 'static> FromReflect for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::List(ref_list) = reflect.reflect_ref() {
            let mut new_list = Self::with_capacity(ref_list.len());
            for field in ref_list.iter() {
                new_list.push(<T as smallvec::Array>::Item::from_reflect(field)?);
            }
            Some(new_list)
        } else {
            None
        }
    }
}

impl<T: smallvec::Array + Send + Sync + 'static> GetTypeRegistration for SmallVec<T>
where
    T::Item: FromReflect,
{
    fn get_type_registration() -> TypeRegistration {
        let mut registration = TypeRegistration::of::<SmallVec<T>>();
        registration.insert::<ReflectFromPtr>(FromType::<SmallVec<T>>::from_type());
        registration
    }
}
