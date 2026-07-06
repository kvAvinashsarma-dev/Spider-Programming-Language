//! The standard library registry — the single source of truth.
//!
//! Every stdlib module function, constant, method, and builtin is described
//! here once: signature, documentation, and required capability. The checker
//! types calls against it, the Silk VM dispatches by it, and the SRS M4 exit
//! criterion (>= 90% doc coverage) is a test over it — so the docs, the
//! types, and the runtime can never drift apart.

use crate::ty::Ty;

/// The capability names a program may declare in `web.toml`.
pub const KNOWN_CAPABILITIES: &[&str] = &["fs", "net", "env", "exec", "ai"];

/// Who is allowed to use capability-gated modules.
#[derive(Debug, Clone)]
pub enum CapPolicy {
    /// No policy (embedding, corpus tests). CLI paths always pass a real one.
    AllowAll,
    /// Safe Mode: only these capabilities (possibly none) are granted.
    Only(std::collections::HashSet<String>),
}

impl CapPolicy {
    pub fn none() -> CapPolicy {
        CapPolicy::Only(std::collections::HashSet::new())
    }

    pub fn allows(&self, cap: &str) -> bool {
        match self {
            CapPolicy::AllowAll => true,
            CapPolicy::Only(set) => set.contains(cap),
        }
    }
}

pub struct ModuleFn {
    pub module: &'static str,
    pub name: &'static str,
    pub params: Vec<Ty>,
    pub ret: Ty,
    pub doc: &'static str,
}

pub struct ModuleConst {
    pub module: &'static str,
    pub name: &'static str,
    pub ty: Ty,
    pub doc: &'static str,
}

pub struct MethodDoc {
    pub receiver: &'static str,
    pub name: &'static str,
    pub doc: &'static str,
}

/// The capability a module needs, if any.
pub fn module_capability(module: &str) -> Option<&'static str> {
    match module {
        "files" => Some("fs"),
        "net" | "http" => Some("net"),
        "env" => Some("env"),
        "exec" => Some("exec"),
        "ai" => Some("ai"),
        _ => None,
    }
}

fn t() -> Ty {
    Ty::Rigid("T".into())
}

pub fn module_fns() -> Vec<ModuleFn> {
    use Ty::*;
    let f = |module, name, params, ret, doc| ModuleFn {
        module,
        name,
        params,
        ret,
        doc,
    };
    vec![
        // math — pure, no capability.
        f("math", "sqrt", vec![Float], Float, "The square root of a Float."),
        f("math", "pow", vec![Float, Float], Float, "Raises the first Float to the power of the second."),
        f("math", "abs", vec![t()], t(), "The distance from zero — always positive."),
        f("math", "min", vec![t(), t()], t(), "The smaller of two comparable values."),
        f("math", "max", vec![t(), t()], t(), "The larger of two comparable values."),
        f("math", "floor", vec![Float], Int, "Rounds a Float down to the nearest whole number."),
        f("math", "round", vec![Float], Int, "Rounds a Float to the nearest whole number."),
        // random — seedable and deterministic, for classrooms.
        f("random", "seed", vec![Int], Unit, "Sets the random sequence's starting point; the same seed always gives the same sequence."),
        f("random", "int", vec![Int, Int], Int, "A random whole number between the two bounds, inclusive."),
        f("random", "float", vec![], Float, "A random Float from 0 (inclusive) to 1 (exclusive)."),
        // files — needs the fs capability.
        f("files", "read_text", vec![Text], Outcome(Box::new(Text)), "Reads a whole file as Text; Fails if the file cannot be read."),
        f("files", "write_text", vec![Text, Text], Outcome(Box::new(Bool)), "Writes Text to a file (path, content), replacing what was there; Fails if it cannot write."),
        f("files", "exists", vec![Text], Bool, "True if a file or folder exists at this path."),
        f("files", "list", vec![Text], Outcome(Box::new(List(Box::new(Text)))), "The names inside a folder, sorted; Fails if the folder cannot be read."),
    ]
}

pub fn module_consts() -> Vec<ModuleConst> {
    vec![ModuleConst {
        module: "math",
        name: "pi",
        ty: Ty::Float,
        doc: "The circle constant, about 3.14159.",
    }]
}

pub fn find_module_fn(module: &str, name: &str) -> Option<ModuleFn> {
    module_fns()
        .into_iter()
        .find(|m| m.module == module && m.name == name)
}

pub fn find_module_const(module: &str, name: &str) -> Option<ModuleConst> {
    module_consts()
        .into_iter()
        .find(|m| m.module == module && m.name == name)
}

pub fn module_member_names(module: &str) -> Vec<String> {
    module_fns()
        .iter()
        .filter(|m| m.module == module)
        .map(|m| m.name.to_string())
        .chain(
            module_consts()
                .iter()
                .filter(|m| m.module == module)
                .map(|m| m.name.to_string()),
        )
        .collect()
}

/// True if the registry implements anything for this module yet.
pub fn module_is_implemented(module: &str) -> bool {
    module_fns().iter().any(|m| m.module == module)
        || module_consts().iter().any(|m| m.module == module)
}

/// Built-in method signatures per receiver type (the checker's dispatch).
pub fn method_sig(base: &Ty, name: &str) -> Option<(Vec<Ty>, Ty)> {
    match base {
        Ty::Int => match name {
            "to_float" => Some((vec![], Ty::Float)),
            "abs" => Some((vec![], Ty::Int)),
            _ => None,
        },
        Ty::Float => match name {
            "to_int" => Some((vec![], Ty::Int)),
            "round" | "floor" => Some((vec![], Ty::Int)),
            "abs" => Some((vec![], Ty::Float)),
            _ => None,
        },
        Ty::Text => match name {
            "length" => Some((vec![], Ty::Int)),
            "upper" | "lower" | "trim" => Some((vec![], Ty::Text)),
            "contains" => Some((vec![Ty::Text], Ty::Bool)),
            "split" => Some((vec![Ty::Text], Ty::List(Box::new(Ty::Text)))),
            _ => None,
        },
        Ty::List(t) => match name {
            "length" => Some((vec![], Ty::Int)),
            "first" | "last" => Some((vec![], Ty::Maybe(t.clone()))),
            "sort" | "reverse" => Some((vec![], Ty::List(t.clone()))),
            "push" => Some((vec![(**t).clone()], Ty::Unit)),
            "contains" => Some((vec![(**t).clone()], Ty::Bool)),
            _ => None,
        },
        Ty::Map(k, v) => match name {
            "length" => Some((vec![], Ty::Int)),
            "keys" => Some((vec![], Ty::List(k.clone()))),
            "values" => Some((vec![], Ty::List(v.clone()))),
            "has" => Some((vec![(**k).clone()], Ty::Bool)),
            _ => None,
        },
        _ => None,
    }
}

pub fn method_docs() -> Vec<MethodDoc> {
    let d = |receiver, name, doc| MethodDoc {
        receiver,
        name,
        doc,
    };
    vec![
        d("Int", "to_float", "This whole number as a Float."),
        d("Int", "abs", "The distance from zero — always positive."),
        d(
            "Float",
            "to_int",
            "Cuts off the decimal part, keeping the whole number.",
        ),
        d("Float", "round", "The nearest whole number."),
        d("Float", "floor", "Rounds down to the nearest whole number."),
        d("Float", "abs", "The distance from zero — always positive."),
        d("Text", "length", "How many characters the text has."),
        d("Text", "upper", "The same text in UPPERCASE."),
        d("Text", "lower", "The same text in lowercase."),
        d("Text", "trim", "The text without spaces at either end."),
        d(
            "Text",
            "contains",
            "True if the other text appears inside this one.",
        ),
        d(
            "Text",
            "split",
            "Cuts the text at every separator into a List of Text.",
        ),
        d("List", "length", "How many items the list holds."),
        d(
            "List",
            "first",
            "Maybe the first item — None when the list is empty.",
        ),
        d(
            "List",
            "last",
            "Maybe the last item — None when the list is empty.",
        ),
        d(
            "List",
            "sort",
            "A new list with the items in order, smallest first.",
        ),
        d(
            "List",
            "reverse",
            "A new list with the items in the opposite order.",
        ),
        d("List", "push", "Adds an item to the end of this list."),
        d("List", "contains", "True if the item appears in the list."),
        d("Map", "length", "How many key-value pairs the map holds."),
        d("Map", "keys", "All the keys, in the order they were added."),
        d(
            "Map",
            "values",
            "All the values, in the order they were added.",
        ),
        d("Map", "has", "True if the map holds this key."),
    ]
}

/// Global builtin functions (available everywhere, no import).
pub struct BuiltinFn {
    pub name: &'static str,
    pub doc: &'static str,
}

pub fn builtin_fns() -> Vec<BuiltinFn> {
    vec![BuiltinFn {
        name: "expect",
        doc: "Checks that two values are equal; stops the test with a clear message when they are not. The heart of `spider test`.",
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SRS M4 exit criterion: >= 90% of the stdlib is documented.
    /// Our actual bar is 100%, enforced here forever.
    #[test]
    fn stdlib_doc_coverage_is_total() {
        let mut total = 0usize;
        let mut documented = 0usize;
        for f in module_fns() {
            total += 1;
            if !f.doc.trim().is_empty() {
                documented += 1;
            }
        }
        for c in module_consts() {
            total += 1;
            if !c.doc.trim().is_empty() {
                documented += 1;
            }
        }
        for m in method_docs() {
            total += 1;
            if !m.doc.trim().is_empty() {
                documented += 1;
            }
        }
        for b in builtin_fns() {
            total += 1;
            if !b.doc.trim().is_empty() {
                documented += 1;
            }
        }
        assert!(total >= 35, "registry looks truncated: {total} entries");
        assert_eq!(
            documented, total,
            "every stdlib entry must be documented ({documented}/{total})"
        );
    }

    /// Every documented method actually dispatches, and vice versa can't
    /// drift silently: the doc list and the signature table must agree.
    #[test]
    fn method_docs_match_dispatch() {
        for m in method_docs() {
            let sample = match m.receiver {
                "Int" => Ty::Int,
                "Float" => Ty::Float,
                "Text" => Ty::Text,
                "List" => Ty::List(Box::new(Ty::Int)),
                "Map" => Ty::Map(Box::new(Ty::Text), Box::new(Ty::Int)),
                other => panic!("unknown receiver {other}"),
            };
            assert!(
                method_sig(&sample, m.name).is_some(),
                "{}.{} is documented but does not dispatch",
                m.receiver,
                m.name
            );
        }
    }
}
