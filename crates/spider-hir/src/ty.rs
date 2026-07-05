//! Spider's type representation and the unifier.
//!
//! `Any` is the error/unknown type: it unifies with everything, so one
//! mistake produces one diagnostic instead of a cascade. `Rigid` is a
//! signature type parameter (`T`) inside its own function; at call sites
//! rigids are instantiated to fresh inference variables.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Text,
    /// The type of statements and functions that return nothing.
    Unit,
    Range,
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Maybe(Box<Ty>),
    Outcome(Box<Ty>),
    Record(String),
    Choice(String),
    Fn(Vec<Ty>, Box<Ty>),
    Rigid(String),
    Var(u32),
    Any,
}

pub struct Unifier {
    subst: Vec<Option<Ty>>,
}

impl Unifier {
    pub fn new() -> Self {
        Unifier { subst: Vec::new() }
    }

    pub fn fresh(&mut self) -> Ty {
        self.subst.push(None);
        Ty::Var((self.subst.len() - 1) as u32)
    }

    /// Follows variable bindings one level deep.
    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut cur = t.clone();
        while let Ty::Var(i) = cur {
            match &self.subst[i as usize] {
                Some(bound) => cur = bound.clone(),
                None => break,
            }
        }
        cur
    }

    /// Fully resolves a type for display and final decisions.
    pub fn resolve(&self, t: &Ty) -> Ty {
        match self.shallow(t) {
            Ty::List(t) => Ty::List(Box::new(self.resolve(&t))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.resolve(&k)), Box::new(self.resolve(&v))),
            Ty::Maybe(t) => Ty::Maybe(Box::new(self.resolve(&t))),
            Ty::Outcome(t) => Ty::Outcome(Box::new(self.resolve(&t))),
            Ty::Fn(ps, r) => Ty::Fn(
                ps.iter().map(|p| self.resolve(p)).collect(),
                Box::new(self.resolve(&r)),
            ),
            other => other,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(i) => i == v,
            Ty::List(t) | Ty::Maybe(t) | Ty::Outcome(t) => self.occurs(v, &t),
            Ty::Map(k, val) => self.occurs(v, &k) || self.occurs(v, &val),
            Ty::Fn(ps, r) => ps.iter().any(|p| self.occurs(v, p)) || self.occurs(v, &r),
            _ => false,
        }
    }

    /// True on success. On failure nothing is bound, and the caller reports.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let a = self.shallow(a);
        let b = self.shallow(b);
        match (&a, &b) {
            (Ty::Any, _) | (_, Ty::Any) => true,
            (Ty::Var(i), _) => {
                if let Ty::Var(j) = b {
                    if *i == j {
                        return true;
                    }
                }
                if self.occurs(*i, &b) {
                    return false;
                }
                self.subst[*i as usize] = Some(b);
                true
            }
            (_, Ty::Var(_)) => self.unify(&b, &a),
            (Ty::Int, Ty::Int)
            | (Ty::Float, Ty::Float)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Text, Ty::Text)
            | (Ty::Unit, Ty::Unit)
            | (Ty::Range, Ty::Range) => true,
            (Ty::List(x), Ty::List(y))
            | (Ty::Maybe(x), Ty::Maybe(y))
            | (Ty::Outcome(x), Ty::Outcome(y)) => self.unify(x, y),
            (Ty::Map(k1, v1), Ty::Map(k2, v2)) => self.unify(k1, k2) && self.unify(v1, v2),
            (Ty::Record(x), Ty::Record(y)) | (Ty::Choice(x), Ty::Choice(y)) => x == y,
            (Ty::Rigid(x), Ty::Rigid(y)) => x == y,
            (Ty::Fn(p1, r1), Ty::Fn(p2, r2)) => {
                p1.len() == p2.len()
                    && p1
                        .iter()
                        .zip(p2.iter())
                        .all(|(x, y)| self.unify(x, y))
                    && self.unify(r1, r2)
            }
            _ => false,
        }
    }

    /// Human display in Spider's own type syntax.
    pub fn show(&self, t: &Ty) -> String {
        match self.resolve(t) {
            Ty::Int => "Int".into(),
            Ty::Float => "Float".into(),
            Ty::Bool => "Bool".into(),
            Ty::Text => "Text".into(),
            Ty::Unit => "Nothing".into(),
            Ty::Range => "Range".into(),
            Ty::List(t) => format!("List of {}", self.show(&t)),
            Ty::Map(k, v) => format!("Map of {} to {}", self.show(&k), self.show(&v)),
            Ty::Maybe(t) => format!("Maybe of {}", self.show(&t)),
            Ty::Outcome(t) => format!("Outcome of {}", self.show(&t)),
            Ty::Record(n) | Ty::Choice(n) => n,
            Ty::Fn(ps, r) => {
                let params: Vec<String> = ps.iter().map(|p| self.show(p)).collect();
                format!("fn({}) -> {}", params.join(", "), self.show(&r))
            }
            Ty::Rigid(n) => n,
            Ty::Var(_) => "_".into(),
            Ty::Any => "_".into(),
        }
    }
}

/// Replaces signature type parameters by the given mapping (used to
/// instantiate a generic function's signature at a call site).
pub fn subst_rigids(t: &Ty, map: &HashMap<String, Ty>) -> Ty {
    match t {
        Ty::Rigid(n) => map.get(n).cloned().unwrap_or_else(|| t.clone()),
        Ty::List(t) => Ty::List(Box::new(subst_rigids(t, map))),
        Ty::Map(k, v) => Ty::Map(Box::new(subst_rigids(k, map)), Box::new(subst_rigids(v, map))),
        Ty::Maybe(t) => Ty::Maybe(Box::new(subst_rigids(t, map))),
        Ty::Outcome(t) => Ty::Outcome(Box::new(subst_rigids(t, map))),
        Ty::Fn(ps, r) => Ty::Fn(
            ps.iter().map(|p| subst_rigids(p, map)).collect(),
            Box::new(subst_rigids(r, map)),
        ),
        other => other.clone(),
    }
}

pub fn collect_rigids(t: &Ty, out: &mut Vec<String>) {
    match t {
        Ty::Rigid(n) => {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        Ty::List(t) | Ty::Maybe(t) | Ty::Outcome(t) => collect_rigids(t, out),
        Ty::Map(k, v) => {
            collect_rigids(k, out);
            collect_rigids(v, out);
        }
        Ty::Fn(ps, r) => {
            for p in ps {
                collect_rigids(p, out);
            }
            collect_rigids(r, out);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_basics() {
        let mut u = Unifier::new();
        let v = u.fresh();
        assert!(u.unify(&v, &Ty::Int));
        assert_eq!(u.resolve(&v), Ty::Int);
        assert!(!u.unify(&Ty::Int, &Ty::Float));
        assert!(u.unify(&Ty::Any, &Ty::Float));
        let f = u.fresh();
        assert!(u.unify(&Ty::List(Box::new(Ty::Int)), &Ty::List(Box::new(f))));
    }

    #[test]
    fn occurs_check_blocks_infinite_types() {
        let mut u = Unifier::new();
        let v = u.fresh();
        let list_v = Ty::List(Box::new(v.clone()));
        assert!(!u.unify(&v, &list_v));
    }

    #[test]
    fn show_uses_spider_syntax() {
        let u = Unifier::new();
        let t = Ty::Map(Box::new(Ty::Text), Box::new(Ty::List(Box::new(Ty::Int))));
        assert_eq!(u.show(&t), "Map of Text to List of Int");
    }
}
