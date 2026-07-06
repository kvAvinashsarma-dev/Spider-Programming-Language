//! Runtime values.
//!
//! Spider has value semantics (LDD §9.1): assignment behaves like a copy.
//! The implementation is copy-on-write — containers share an `Rc` until a
//! mutation, when `Rc::make_mut` clones only if the value is shared. Maps are
//! insertion-ordered vectors: deterministic iteration is a language guarantee
//! (SRS NFR-1), and honest O(n) lookup is fine at M3 scale.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct RecordShape {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RecordVal {
    pub shape: Rc<RecordShape>,
    pub fields: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct VariantVal {
    pub tag: Rc<str>,
    pub fields: Vec<Value>,
}

#[derive(Debug, Clone)]
pub enum Iter {
    List { items: Rc<Vec<Value>>, idx: usize },
    Range { cur: i64, end: i64 },
}

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(Rc<String>),
    List(Rc<Vec<Value>>),
    Map(Rc<Vec<(Value, Value)>>),
    Range(i64, i64),
    Record(Rc<RecordVal>),
    Variant(Rc<VariantVal>),
    FnRef(u32),
    Iter(Rc<RefCell<Iter>>),
}

impl Value {
    pub fn text(s: impl Into<String>) -> Value {
        Value::Text(Rc::new(s.into()))
    }

    pub fn variant(tag: &str, fields: Vec<Value>) -> Value {
        Value::Variant(Rc::new(VariantVal {
            tag: Rc::from(tag),
            fields,
        }))
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Unit => "Nothing",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Text(_) => "Text",
            Value::List(_) => "List",
            Value::Map(_) => "Map",
            Value::Range(..) => "Range",
            Value::Record(_) => "a record",
            Value::Variant(_) => "a choice value",
            Value::FnRef(_) => "a function",
            Value::Iter(_) => "an iterator",
        }
    }
}

pub fn fmt_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// Human display. `quote_text` is true inside containers, so
/// `say "hi"` prints `hi` but `say ["hi"]` prints `["hi"]`.
pub fn display(v: &Value, quote_text: bool) -> String {
    match v {
        Value::Unit => "nothing".into(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => fmt_float(*f),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => {
            if quote_text {
                format!("{:?}", s.as_str())
            } else {
                s.as_str().to_string()
            }
        }
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|i| display(i, true)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", display(k, true), display(v, true)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Range(a, b) => format!("{a} to {b}"),
        Value::Record(r) => {
            let parts: Vec<String> = r
                .shape
                .fields
                .iter()
                .zip(r.fields.iter())
                .map(|(name, val)| format!("{name}: {}", display(val, true)))
                .collect();
            format!("{}({})", r.shape.name, parts.join(", "))
        }
        Value::Variant(var) => {
            if var.fields.is_empty() {
                var.tag.to_string()
            } else {
                let parts: Vec<String> = var.fields.iter().map(|f| display(f, true)).collect();
                format!("{}({})", var.tag, parts.join(", "))
            }
        }
        Value::FnRef(_) => "a function".into(),
        Value::Iter(_) => "an iterator".into(),
    }
}

pub fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Unit, Value::Unit) => true,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Text(x), Value::Text(y)) => x == y,
        (Value::Range(a1, b1), Value::Range(a2, b2)) => a1 == a2 && b1 == b2,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| value_eq(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| value_eq(k, k2))
                        .is_some_and(|(_, v2)| value_eq(v, v2))
                })
        }
        (Value::Record(x), Value::Record(y)) => {
            x.shape.name == y.shape.name
                && x.fields
                    .iter()
                    .zip(y.fields.iter())
                    .all(|(a, b)| value_eq(a, b))
        }
        (Value::Variant(x), Value::Variant(y)) => {
            x.tag == y.tag
                && x.fields.len() == y.fields.len()
                && x.fields
                    .iter()
                    .zip(y.fields.iter())
                    .all(|(a, b)| value_eq(a, b))
        }
        (Value::FnRef(x), Value::FnRef(y)) => x == y,
        _ => false,
    }
}

/// Ordering for `< <= > >=` and `.sort()`. Only Int, Float, Text, Bool order.
pub fn value_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => Some(x.total_cmp(y)),
        (Value::Text(x), Value::Text(y)) => Some(x.as_str().cmp(y.as_str())),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}
