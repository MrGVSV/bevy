use crate::serde::ser::error_utils::make_custom_error;
use crate::serde::{SerializationData, TypedReflectSerializer};
use crate::{Struct, TypeRegistry};
use serde::ser::SerializeStruct;
use serde::Serialize;

/// A serializer for [`Struct`] values.
pub(super) struct StructSerializer<'a> {
    struct_value: &'a dyn Struct,
    registry: &'a TypeRegistry,
}

impl<'a> StructSerializer<'a> {
    pub fn new(struct_value: &'a dyn Struct, registry: &'a TypeRegistry) -> Self {
        Self {
            struct_value,
            registry,
        }
    }
}

impl<'a> Serialize for StructSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_info = self
            .struct_value
            .get_represented_type_info()
            .ok_or_else(|| {
                make_custom_error(format_args!(
                    "cannot get type info for `{}`",
                    self.struct_value.reflect_type_path()
                ))
            })?;

        let struct_info = type_info.as_struct().map_err(make_custom_error)?;

        let serialization_data = self
            .registry
            .get(type_info.type_id())
            .and_then(|registration| registration.data::<SerializationData>());
        let ignored_len = serialization_data.map(SerializationData::len).unwrap_or(0);
        let mut state = serializer.serialize_struct(
            struct_info.type_path_table().ident().unwrap(),
            self.struct_value.field_len() - ignored_len,
        )?;

        for (index, value) in self.struct_value.iter_fields().enumerate() {
            if serialization_data
                .map(|data| data.is_field_skipped(index))
                .unwrap_or(false)
            {
                continue;
            }

            let info = struct_info.field_at(index).unwrap().type_info();

            let key = struct_info.field_at(index).unwrap().name();
            state.serialize_field(
                key,
                &TypedReflectSerializer::new_internal(value, info, self.registry),
            )?;
        }
        state.end()
    }
}
