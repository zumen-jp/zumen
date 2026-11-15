//! 構造のあるプロパティ。元の番号を除き、親の位置・元値・単位を同じ並びで書く。

use super::{Canon, Encode};
use crate::{
    DetailedProperty, DetailedPropertySet, PropertyChoices, PropertyDetailValue, PropertyDetails,
};

impl Encode for PropertyDetails {
    fn encode(&self, out: &mut Canon<'_>) {
        self.sets.encode(out);
    }
}
impl Encode for DetailedPropertySet {
    fn encode(&self, out: &mut Canon<'_>) {
        self.name.encode(out);
        self.description.encode(out);
        self.properties.encode(out);
    }
}
impl Encode for DetailedProperty {
    fn encode(&self, out: &mut Canon<'_>) {
        self.parent.encode(out);
        self.name.encode(out);
        self.description.encode(out);
        self.value.encode(out);
    }
}
impl Encode for PropertyChoices {
    fn encode(&self, out: &mut Canon<'_>) {
        self.name.encode(out);
        self.values.encode(out);
        self.unit.encode(out);
    }
}
impl Encode for PropertyDetailValue {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Single { value, unit } => {
                0u8.encode(out);
                value.encode(out);
                unit.encode(out);
            }
            Self::List { values, unit } => {
                1u8.encode(out);
                values.encode(out);
                unit.encode(out);
            }
            Self::Enumerated { values, choices } => {
                2u8.encode(out);
                values.encode(out);
                choices.encode(out);
            }
            Self::Bounded {
                lower,
                upper,
                set_point,
                unit,
            } => {
                3u8.encode(out);
                lower.encode(out);
                upper.encode(out);
                set_point.encode(out);
                unit.encode(out);
            }
            Self::Complex { usage_name } => {
                4u8.encode(out);
                usage_name.encode(out);
            }
            Self::Unsupported { kind } => {
                5u8.encode(out);
                kind.encode(out);
            }
        }
    }
}
