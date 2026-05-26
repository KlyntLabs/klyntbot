use specta::{datatype::DataType, Generics, Type, TypeMap};

pub struct JsonValue;

impl Type for JsonValue {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        DataType::Unknown
    }
}

pub struct Timestamp;

impl Type for Timestamp {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}

pub struct TimestampTuple;

impl Type for TimestampTuple {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        specta::internal::construct::tuple(vec![
            DataType::Primitive(specta::datatype::PrimitiveType::String),
            DataType::Primitive(specta::datatype::PrimitiveType::String),
        ])
        .into()
    }
}
