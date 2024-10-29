use core::num;
use std::{any::Any, collections::HashMap};

use json::JsonValue;

#[derive(Debug)]
pub enum NBT {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<NBT>),
    Compound(Vec<NamedTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

#[derive(Debug)]
pub struct NamedTag {
    pub tag: NBT,
    pub name: String,
}

impl NBT {
    pub fn type_id(&self) -> u8 {
        match self {
            NBT::End => 0,
            NBT::Byte(_) => 1,
            NBT::Short(_) => 2,
            NBT::Int(_) => 3,
            NBT::Long(_) => 4,
            NBT::Float(_) => 5,
            NBT::Double(_) => 6,
            NBT::ByteArray(_) => 7,
            NBT::String(_) => 8,
            NBT::List(_) => 9,
            NBT::Compound(_) => 10,
            NBT::IntArray(_) => 11,
            NBT::LongArray(_) => 12,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![];
        match &self {
            NBT::End => {
                return vec![0x0];
            }
            NBT::Byte(b) => {
                return out;
            }
            NBT::Short(s) => {
                out.extend_from_slice(&s.to_be_bytes());
                return out;
            }
            NBT::Int(i) => {
                out.extend_from_slice(&i.to_be_bytes());
                return out;
            }
            NBT::Long(l) => {
                out.extend_from_slice(&l.to_be_bytes());
                return out;
            }
            NBT::Float(f) => {
                out.extend_from_slice(&f.to_be_bytes());
                return out;
            }
            NBT::Double(d) => {
                out.extend_from_slice(&d.to_be_bytes());
                return out;
            }
            NBT::ByteArray(vec) => {
                out.extend_from_slice(&(vec.len() as u16).to_be_bytes());
                out.extend_from_slice(&vec);
                return out;
            }
            NBT::String(s) => {
                out.extend_from_slice(&(s.as_bytes().len() as u16).to_be_bytes());
                out.extend_from_slice(&s.as_bytes());
                return out;
            }
            NBT::List(vec) => {
                let type_id = vec.first().map(|t| t.type_id()).unwrap_or(0);
                out.push(type_id);
                out.extend_from_slice(&(vec.len() as i32).to_be_bytes());
                for nbt in vec {
                    assert!(nbt.type_id() == type_id);
                    out.extend_from_slice(&nbt.to_bytes());
                }
                return out;
            }
            NBT::Compound(vec) => {
                for tag in vec {
                    out.extend_from_slice(&tag.to_bytes());
                }
                out.push(0x0);
                return out;
            }
            NBT::IntArray(vec) => {
                out.extend_from_slice(&(vec.len() as i32).to_be_bytes());
                for i in vec {
                    out.extend_from_slice(&i.to_be_bytes());
                }
                // out.push(0x0);
                return out;
            }
            NBT::LongArray(vec) => {
                out.extend_from_slice(&(vec.len() as i32).to_be_bytes());
                for l in vec {
                    out.extend_from_slice(&l.to_be_bytes());
                }
                // out.push(0x0);
                return out;
            }
        }
    }
}

impl NamedTag {
    pub fn new(name: impl Into<String>, tag: NBT) -> Self {
        Self {
            tag,
            name: name.into(),
        }
    }

    pub fn to_bytes_internal(&self, named: bool) -> Vec<u8> {
        if self.tag.type_id() == 0 {
            return vec![0];
        }

        let mut out = vec![self.tag.type_id()];
        if named {
            out.extend_from_slice(&(self.name.as_bytes().len() as u16).to_be_bytes());
            out.extend_from_slice(&self.name.as_bytes());
        }
        out.extend_from_slice(&self.tag.to_bytes());

        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes_internal(true)

        // match &self.tag {
        //     NBT::End => {
        //         return vec![0x0];
        //     },
        //     NBT::Byte(b) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.push(*b as _);
        //         return out;
        //     },
        //     NBT::Short(s) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&s.to_be_bytes());
        //         return out;
        //     },
        //     NBT::Int(i) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&i.to_be_bytes());
        //         return out;
        //     },
        //     NBT::Long(l) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&l.to_be_bytes());
        //         return out;
        //     },
        //     NBT::Float(f) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&f.to_be_bytes());
        //         return out;
        //     },
        //     NBT::Double(d) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&d.to_be_bytes());
        //         return out;
        //     },
        //     NBT::ByteArray(vec) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&(vec.len() as u16).to_be_bytes());
        //         out.extend_from_slice(&vec);
        //         return out;
        //     },
        //     NBT::String(s) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         out.extend_from_slice(&(s.as_bytes().len() as u16).to_be_bytes());
        //         out.extend_from_slice(&s.as_bytes());
        //         return out;
        //     },
        //     NBT::List(vec) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         for nbt in vec {
        //             out.extend_from_slice();
        //         }
        //         out.push(0x00);
        //         return out;
        //     },
        //     NBT::Compound(vec) => {
        //         let mut out = vec![self.tag.type_id()];
        //         add_name_if_needed(&mut out);
        //         // out.extend_from_slice(&(vec.len() as u16).to_be_bytes());
        //         for tag in vec {
        //             out.extend_from_slice(&tag.to_bytes());
        //         }
        //         out.push(0x0);
        //         return out;
        //     },
        //     NBT::IntArray(vec) => todo!(),
        //     NBT::LongArray(vec) => todo!(),
        // }
        // out
    }

    // pub fn to_bytes(&self) -> Vec<u8> {
    // return self.to_bytes_internal(true)
    // }
}

fn from_json_object(data: json::object::Object) -> NBT {
    let mut list = vec![];
    for (k, v) in data.iter() {
        let n = match v {
            JsonValue::Null => unimplemented!(),
            JsonValue::Short(short) => NBT::String(short.as_str().to_string()),
            JsonValue::String(s) => NBT::String(s.to_string()),
            JsonValue::Number(number) => {
                let f = f64::from(number.clone());
                if f.fract() == 0.0 {
                    NBT::Int(f as i32)
                } else {
                    NBT::Float(f as f32)
                }
            }
            JsonValue::Boolean(b) => NBT::Byte(*b as i8),
            JsonValue::Object(object) => from_json_object(object.clone()),
            JsonValue::Array(vec) => from_json_array(vec.clone()),
        };
        list.push(NamedTag::new(k.to_string(), n));
    }
    NBT::Compound(list)
}

fn from_json_array(data: Vec<JsonValue>) -> NBT {
    let mut list = vec![];
    for v in data {
        let n = match v {
            JsonValue::Null => unimplemented!(),
            JsonValue::Short(short) => NBT::String(short.as_str().to_string()),
            JsonValue::String(s) => NBT::String(s),
            JsonValue::Number(number) => NBT::Int(number.as_fixed_point_i64(0).unwrap() as i32),
            JsonValue::Boolean(b) => NBT::Byte(b as i8),
            JsonValue::Object(object) => from_json_object(object),
            JsonValue::Array(vec) => from_json_array(vec),
        };
        list.push(n);
    }
    NBT::List(list)
}

pub fn from_json(s: &str) -> NamedTag {
    let data = json::parse(s).unwrap();

    match data {
        JsonValue::Object(o) => NamedTag::new("", from_json_object(o)),
        _ => unimplemented!(),
    }
}
