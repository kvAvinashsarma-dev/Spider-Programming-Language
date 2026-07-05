//! Name resolution and type checking over the CST.
//!
//! Two passes over the file:
//!   A. collect declaration *names* (records, choices, shapes, functions);
//!   B. resolve signatures and field types, then check every body.
//!
//! Design rules (ADR-012):
//! - `Any` silences cascades: one mistake, one diagnostic.
//! - Unannotated non-public parameters are `Any` (cross-function inference
//!   is a later milestone); public parameters must be annotated (E0208).
//! - Single uppercase letters (`T`, `U`, …) in signatures are type
//!   parameters; call sites instantiate them fresh.
//! - The trailing match/expression of a non-Nothing function is its implicit
//!   return value.

use crate::span::SpanMap;
use crate::stdlib::{self, CapPolicy};
use crate::ty::{collect_rigids, subst_rigids, Ty, Unifier};
use spider_syntax::{Diagnostic, Node, Parse, SyntaxKind as K};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const STD_MODULES: &[&str] = &[
    "math", "time", "random", "json", "regex", "test", "files", "net", "env", "exec", "ai",
];
const BUILTIN_CONSTRAINTS: &[&str] = &["Comparable", "Equatable", "Printable"];

#[derive(Clone)]
pub(crate) struct FnSig {
    pub(crate) params: Vec<Ty>,
    pub(crate) ret: Ty,
}

struct Local {
    ty: Ty,
    mutable: bool,
    used: bool,
    span: (usize, usize),
}

/// A public function another module may call: (params, return, is_public).
pub type ExportedFn = (Vec<Ty>, Ty, bool);

/// One module of a multi-file project, ready for `check_project`.
pub struct ProjectModule<'a> {
    /// The module's binding name (last path segment), e.g. `helpers`.
    pub name: String,
    pub parse: &'a Parse,
    /// use-alias -> target module name, for imports that resolved to files.
    pub imports: HashMap<String, String>,
}

/// Checks a whole project. Types (records/choices) are project-global;
/// functions cross module boundaries only via `alias.fn(...)` and only when
/// `public` (ADR-015). Returns diagnostics tagged with the module index.
pub fn check_project(
    modules: &[ProjectModule],
    entry: usize,
    policy: &CapPolicy,
) -> Vec<(usize, Diagnostic)> {
    let mut out: Vec<(usize, Diagnostic)> = Vec::new();

    // Stage 1: merge type names across modules; duplicate type names across
    // modules are reported on the later module.
    let mut type_owner: HashMap<String, (usize, bool)> = HashMap::new(); // name -> (module, is_record)
    for (i, m) in modules.iter().enumerate() {
        for n in m.parse.root.child_nodes() {
            if matches!(n.kind, K::RecordDecl | K::ChoiceDecl) {
                if let Some(name) = n.find_token(K::Ident).map(|t| t.text.clone()) {
                    if type_owner.contains_key(&name) {
                        let spans = SpanMap::build(&m.parse.root);
                        let (s, e) = spans.of(n);
                        out.push((
                            i,
                            Diagnostic::error(
                                "E0203",
                                format!("the type `{name}` already exists in another module of this project"),
                                s,
                                e.saturating_sub(s),
                            ),
                        ));
                    } else {
                        type_owner.insert(name, (i, n.kind == K::RecordDecl));
                    }
                }
            }
        }
    }

    // Stage 2: per-module checkers, seeded with all project type names so
    // cross-module type references classify; each resolves its own
    // signatures (its own E0204/E0208/… fire exactly once, here).
    let mut checkers: Vec<Checker> = Vec::new();
    for (i, m) in modules.iter().enumerate() {
        let mut c = new_checker(m.parse, policy);
        c.known_user_aliases = m.imports.keys().cloned().collect();
        for (tname, (owner, is_record)) in &type_owner {
            if *owner != i {
                if *is_record {
                    c.records.entry(tname.clone()).or_default();
                } else {
                    c.choices.entry(tname.clone()).or_default();
                }
            }
        }
        c.collect_names(&m.parse.root);
        c.collect_signatures(&m.parse.root);
        checkers.push(c);
    }

    // Stage 3: merge resolved type tables, variants, constructors, exports.
    let mut all_records = HashMap::new();
    let mut all_choices = HashMap::new();
    let mut all_variants = HashMap::new();
    let mut ctor_fns: HashMap<String, FnSig> = HashMap::new();
    let mut exports: HashMap<String, HashMap<String, ExportedFn>> = HashMap::new();
    for (m, c) in modules.iter().zip(checkers.iter()) {
        for (k, v) in &c.records {
            if !v.is_empty() || !all_records.contains_key(k) {
                all_records.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &c.choices {
            if !v.is_empty() || !all_choices.contains_key(k) {
                all_choices.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &c.variant_of {
            all_variants.insert(k.clone(), v.clone());
        }
        let mut mod_exports = HashMap::new();
        for (fname, sig) in &c.fns {
            let is_ctor =
                c.records.contains_key(fname) || c.variant_of.contains_key(fname);
            if is_ctor {
                ctor_fns.entry(fname.clone()).or_insert_with(|| sig.clone());
            } else {
                let is_public = c.public_fns.contains(fname);
                mod_exports.insert(fname.clone(), (sig.params.clone(), sig.ret.clone(), is_public));
            }
        }
        exports.insert(m.name.clone(), mod_exports);
    }

    // Stage 4: body-check each module with the merged world in view.
    for (i, (m, mut c)) in modules.iter().zip(checkers.into_iter()).enumerate() {
        c.records = all_records.clone();
        c.choices = all_choices.clone();
        c.variant_of = all_variants.clone();
        for (name, sig) in &ctor_fns {
            c.fns.entry(name.clone()).or_insert_with(|| sig.clone());
        }
        for (alias, target) in &m.imports {
            if let Some(ex) = exports.get(target) {
                c.user_modules.insert(alias.clone(), ex.clone());
            }
        }
        c.is_entry_module = i == entry;
        c.check_top(&m.parse.root);
        c.diags.sort_by_key(|d| d.offset);
        for d in c.diags {
            out.push((i, d));
        }
    }
    out
}

/// Checks with no capability policy — embedding and corpus tests.
/// The CLI always calls `check_parse_caps` with a real policy.
pub fn check_parse(parse: &Parse) -> Vec<Diagnostic> {
    check_parse_caps(parse, &CapPolicy::AllowAll)
}

fn new_checker(parse: &Parse, policy: &CapPolicy) -> Checker {
    Checker {
        spans: SpanMap::build(&parse.root),
        uni: Unifier::new(),
        diags: Vec::new(),
        fns: HashMap::new(),
        records: HashMap::new(),
        choices: HashMap::new(),
        variant_of: HashMap::new(),
        shapes: HashSet::new(),
        modules: HashSet::new(),
        scopes: Vec::new(),
        current_ret: None,
        current_rigids: HashSet::new(),
        span_override: None,
        cap_policy: policy.clone(),
        user_modules: HashMap::new(),
        known_user_aliases: HashSet::new(),
        public_fns: HashSet::new(),
        is_entry_module: true,
    }
}

pub fn check_parse_caps(parse: &Parse, policy: &CapPolicy) -> Vec<Diagnostic> {
    let mut c = Checker {
        spans: SpanMap::build(&parse.root),
        uni: Unifier::new(),
        diags: Vec::new(),
        fns: HashMap::new(),
        records: HashMap::new(),
        choices: HashMap::new(),
        variant_of: HashMap::new(),
        shapes: HashSet::new(),
        modules: HashSet::new(),
        scopes: Vec::new(),
        current_ret: None,
        current_rigids: HashSet::new(),
        span_override: None,
        cap_policy: policy.clone(),
        user_modules: HashMap::new(),
        known_user_aliases: HashSet::new(),
        public_fns: HashSet::new(),
        is_entry_module: true,
    };
    c.collect_names(&parse.root);
    c.collect_signatures(&parse.root);
    c.check_top(&parse.root);
    c.diags.sort_by_key(|d| d.offset);
    c.diags
}

struct Checker {
    spans: SpanMap,
    uni: Unifier,
    diags: Vec<Diagnostic>,
    fns: HashMap<String, FnSig>,
    records: HashMap<String, Vec<(String, Ty)>>,
    /// choice name -> variants (name, field types) in declaration order
    choices: HashMap<String, Vec<(String, Vec<Ty>)>>,
    variant_of: HashMap<String, String>,
    shapes: HashSet<String>,
    modules: HashSet<String>,
    scopes: Vec<HashMap<String, Local>>,
    current_ret: Option<Ty>,
    current_rigids: HashSet<String>,
    /// While checking code parsed out of a `{…}` hole, all diagnostics land
    /// on the enclosing string literal's span.
    span_override: Option<(usize, usize)>,
    cap_policy: CapPolicy,
    /// alias -> exported functions of a sibling module (project mode).
    user_modules: HashMap<String, HashMap<String, ExportedFn>>,
    /// Aliases that resolved to project files (suppresses W0002).
    known_user_aliases: HashSet<String>,
    public_fns: HashSet<String>,
    /// Only the entry module may run top-level statements (E0246).
    is_entry_module: bool,
}

impl Checker {
    // ----- diagnostics -----

    fn err(&mut self, node: &Rc<Node>, code: &'static str, msg: impl Into<String>) {
        let (start, end) = self.span_override.unwrap_or_else(|| self.spans.of(node));
        self.diags
            .push(Diagnostic::error(code, msg, start, end.saturating_sub(start)));
    }

    fn warn(&mut self, span: (usize, usize), code: &'static str, msg: impl Into<String>) {
        self.diags
            .push(Diagnostic::warning(code, msg, span.0, span.1.saturating_sub(span.0)));
    }

    fn err_span(&mut self, span: (usize, usize), code: &'static str, msg: impl Into<String>) {
        self.diags
            .push(Diagnostic::error(code, msg, span.0, span.1.saturating_sub(span.0)));
    }

    fn show(&self, t: &Ty) -> String {
        self.uni.show(t)
    }

    // ----- "did you mean" -----

    fn suggestion(&self, name: &str, candidates: &[String]) -> String {
        let mut best: Option<(usize, &String)> = None;
        for c in candidates {
            if c == name {
                continue;
            }
            let d = levenshtein(name, c);
            let limit = if name.chars().count() <= 3 { 1 } else { 2 };
            if d <= limit && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, c));
            }
        }
        match best {
            Some((_, c)) => format!(" — did you mean `{c}`?"),
            None => String::new(),
        }
    }

    fn visible_value_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.scopes {
            out.extend(s.keys().cloned());
        }
        out.extend(self.fns.keys().cloned());
        out.extend(self.modules.iter().cloned());
        out
    }

    // ----- scopes -----

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            let mut unused: Vec<(String, (usize, usize))> = scope
                .into_iter()
                .filter(|(name, l)| !l.used && !name.starts_with('_'))
                .map(|(name, l)| (name, l.span))
                .collect();
            unused.sort_by_key(|(_, s)| s.0);
            for (name, span) in unused {
                self.warn(span, "W0001", format!("`{name}` is never used"));
            }
        }
    }

    fn define(&mut self, name: &str, ty: Ty, mutable: bool, span: (usize, usize), used: bool) {
        if self.scopes.last().is_some_and(|s| s.contains_key(name)) {
            self.err_span(
                span,
                "E0203",
                format!("`{name}` already exists in this block — assign with `=` to change it"),
            );
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(
                name.to_string(),
                Local {
                    ty,
                    mutable,
                    used,
                    span,
                },
            );
        }
    }

    fn lookup_local(&mut self, name: &str) -> Option<(Ty, bool)> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(l) = scope.get_mut(name) {
                l.used = true;
                return Some((l.ty.clone(), l.mutable));
            }
        }
        None
    }

    // ----- pass A: names -----

    fn collect_names(&mut self, root: &Rc<Node>) {
        for n in root.child_nodes() {
            let name = n.find_token(K::Ident).map(|t| t.text.clone());
            match (n.kind, name) {
                (K::RecordDecl, Some(name)) => {
                    if self.type_exists(&name) {
                        self.err(n, "E0203", format!("the type `{name}` already exists"));
                    }
                    self.records.insert(name, Vec::new());
                }
                (K::ChoiceDecl, Some(name)) => {
                    if self.type_exists(&name) {
                        self.err(n, "E0203", format!("the type `{name}` already exists"));
                    }
                    self.choices.insert(name, Vec::new());
                }
                (K::ShapeDecl, Some(name)) => {
                    self.shapes.insert(name);
                }
                (K::UseDecl, _) => {
                    let segments: Vec<String> = n
                        .child_tokens()
                        .into_iter()
                        .filter(|t| t.kind == K::Ident)
                        .map(|t| t.text.clone())
                        .collect();
                    if let (Some(first), Some(last)) = (segments.first(), segments.last()) {
                        if !STD_MODULES.contains(&first.as_str())
                            && !self.known_user_aliases.contains(last)
                        {
                            let msg = format!(
                                "`{first}` is not a standard-library module{}",
                                self.suggestion(
                                    first,
                                    &STD_MODULES.iter().map(|s| s.to_string()).collect::<Vec<_>>()
                                )
                            );
                            let span = self.spans.of(n);
                            self.warn(span, "W0002", msg);
                        }
                        // Safe Mode (SRS FR-21): a capability-gated module may
                        // only be imported if the capability is granted.
                        if let Some(cap) = stdlib::module_capability(first) {
                            if !self.cap_policy.allows(cap) {
                                let msg = format!(
                                    "using `{first}` needs the `{cap}` capability, which this program was not given — add \"{cap}\" to `allow` in web.toml, or run with `--allow {cap}`"
                                );
                                self.err(n, "E0244", msg);
                            }
                        }
                        self.modules.insert(last.clone());
                    }
                }
                _ => {}
            }
        }
    }

    fn type_exists(&self, name: &str) -> bool {
        self.records.contains_key(name) || self.choices.contains_key(name)
    }

    // ----- pass B: signatures -----

    fn collect_signatures(&mut self, root: &Rc<Node>) {
        for n in root.child_nodes() {
            match n.kind {
                K::RecordDecl => {
                    let Some(name) = n.find_token(K::Ident).map(|t| t.text.clone()) else {
                        continue;
                    };
                    let mut fields = Vec::new();
                    if let Some(block) = n.find_node(K::Block) {
                        for f in block.nodes_of(K::FieldDecl) {
                            let fname = f
                                .find_token(K::Ident)
                                .map(|t| t.text.clone())
                                .unwrap_or_default();
                            let fty = match f.find_node(K::TypeRef) {
                                Some(t) => self.resolve_type(t, &HashSet::new()),
                                None => Ty::Any,
                            };
                            fields.push((fname, fty));
                        }
                    }
                    // The record's constructor: Point(x, y) -> Point.
                    let ctor = FnSig {
                        params: fields.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Ty::Record(name.clone()),
                    };
                    self.add_fn(n, &name, ctor);
                    self.records.insert(name, fields);
                }
                K::ChoiceDecl => {
                    let Some(name) = n.find_token(K::Ident).map(|t| t.text.clone()) else {
                        continue;
                    };
                    let mut variants = Vec::new();
                    if let Some(block) = n.find_node(K::Block) {
                        for v in block.nodes_of(K::VariantDecl) {
                            let vname = v
                                .find_token(K::Ident)
                                .map(|t| t.text.clone())
                                .unwrap_or_default();
                            let mut fields = Vec::new();
                            for p in v.nodes_of(K::Param) {
                                let fty = match p.find_node(K::TypeRef) {
                                    Some(t) => self.resolve_type(t, &HashSet::new()),
                                    None => Ty::Any,
                                };
                                fields.push(fty);
                            }
                            if !fields.is_empty() {
                                let ctor = FnSig {
                                    params: fields.clone(),
                                    ret: Ty::Choice(name.clone()),
                                };
                                self.add_fn(v, &vname, ctor);
                            }
                            self.variant_of.insert(vname.clone(), name.clone());
                            variants.push((vname, fields));
                        }
                    }
                    self.choices.insert(name, variants);
                }
                K::FnDecl => self.collect_fn_sig(n),
                _ => {}
            }
        }
    }

    fn add_fn(&mut self, node: &Rc<Node>, name: &str, sig: FnSig) {
        if self.fns.contains_key(name) {
            self.err(node, "E0203", format!("`{name}` already exists"));
        }
        self.fns.insert(name.to_string(), sig);
    }

    fn where_rigids(&mut self, n: &Rc<Node>) -> HashSet<String> {
        let mut rigids = HashSet::new();
        if let Some(w) = n.find_node(K::WhereClause) {
            let toks: Vec<_> = w
                .child_tokens()
                .into_iter()
                .filter(|t| matches!(t.kind, K::Ident | K::IsKw | K::Comma))
                .cloned()
                .collect();
            let mut i = 0;
            while i + 3 <= toks.len() {
                if toks[i].kind == K::Ident
                    && toks[i + 1].kind == K::IsKw
                    && toks[i + 2].kind == K::Ident
                {
                    rigids.insert(toks[i].text.clone());
                    let constraint = &toks[i + 2].text;
                    if !BUILTIN_CONSTRAINTS.contains(&constraint.as_str())
                        && !self.shapes.contains(constraint)
                    {
                        let shape_names: Vec<String> = self
                            .shapes
                            .iter()
                            .cloned()
                            .chain(BUILTIN_CONSTRAINTS.iter().map(|s| s.to_string()))
                            .collect();
                        let msg = format!(
                            "`{constraint}` is not a known capability{}",
                            self.suggestion(constraint, &shape_names)
                        );
                        self.err(w, "E0241", msg);
                    }
                    i += 3;
                    if i < toks.len() && toks[i].kind == K::Comma {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
        }
        rigids
    }

    fn collect_fn_sig(&mut self, n: &Rc<Node>) {
        let Some(name) = n.find_token(K::Ident).map(|t| t.text.clone()) else {
            return;
        };
        let is_public = n.find_token(K::PublicKw).is_some();
        let rigids = self.where_rigids(n);

        let mut params = Vec::new();
        if let Some(pl) = n.find_node(K::ParamList) {
            for p in pl.nodes_of(K::Param) {
                let pname = p
                    .find_token(K::Ident)
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                let ty = match p.find_node(K::TypeRef) {
                    Some(t) => self.resolve_type(t, &rigids),
                    None => {
                        if is_public && pname != "self" {
                            self.err(
                                p,
                                "E0208",
                                format!("public function `{name}` must give `{pname}` a type"),
                            );
                        }
                        Ty::Any
                    }
                };
                params.push(ty);
            }
        }
        let ret = match n.find_node(K::RetType).and_then(|rt| rt.find_node(K::TypeRef)) {
            Some(t) => self.resolve_type(t, &rigids),
            None => Ty::Unit,
        };
        if is_public {
            self.public_fns.insert(name.clone());
        }
        self.add_fn(n, &name, FnSig { params, ret });
    }

    fn resolve_type(&mut self, t: &Rc<Node>, rigids: &HashSet<String>) -> Ty {
        let nested: Vec<Ty> = t
            .nodes_of(K::TypeRef)
            .into_iter()
            .map(|inner| self.resolve_type(inner, rigids))
            .collect();
        let Some(name) = t.find_token(K::Ident).map(|tk| tk.text.clone()) else {
            // Parenthesized type: (T)
            return nested.into_iter().next().unwrap_or(Ty::Any);
        };
        let arg = |i: usize| nested.get(i).cloned();
        match name.as_str() {
            "Int" => Ty::Int,
            "Float" => Ty::Float,
            "Bool" => Ty::Bool,
            "Text" => Ty::Text,
            "List" => match arg(0) {
                Some(inner) => Ty::List(Box::new(inner)),
                None => {
                    self.err(t, "E0209", "`List` needs an item type — for example `List of Int`");
                    Ty::Any
                }
            },
            "Map" => match (arg(0), arg(1)) {
                (Some(k), Some(v)) => Ty::Map(Box::new(k), Box::new(v)),
                _ => {
                    self.err(
                        t,
                        "E0209",
                        "`Map` needs key and value types — for example `Map of Text to Int`",
                    );
                    Ty::Any
                }
            },
            "Maybe" => match arg(0) {
                Some(inner) => Ty::Maybe(Box::new(inner)),
                None => {
                    self.err(t, "E0209", "`Maybe` needs an inner type — for example `Maybe of Text`");
                    Ty::Any
                }
            },
            "Outcome" => match arg(0) {
                Some(inner) => Ty::Outcome(Box::new(inner)),
                None => {
                    self.err(
                        t,
                        "E0209",
                        "`Outcome` needs a success type — for example `Outcome of Settings`",
                    );
                    Ty::Any
                }
            },
            _ if self.records.contains_key(&name) => Ty::Record(name),
            _ if self.choices.contains_key(&name) => Ty::Choice(name),
            _ if rigids.contains(&name) || self.current_rigids.contains(&name) => Ty::Rigid(name),
            _ if name.chars().count() == 1
                && name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) =>
            {
                Ty::Rigid(name)
            }
            _ => {
                let mut known: Vec<String> = vec![
                    "Int".into(),
                    "Float".into(),
                    "Bool".into(),
                    "Text".into(),
                    "List".into(),
                    "Map".into(),
                    "Maybe".into(),
                    "Outcome".into(),
                ];
                known.extend(self.records.keys().cloned());
                known.extend(self.choices.keys().cloned());
                let msg = format!(
                    "the type `{name}` is not known{}",
                    self.suggestion(&name, &known)
                );
                self.err(t, "E0204", msg);
                Ty::Any
            }
        }
    }

    // ----- bodies -----

    fn check_top(&mut self, root: &Rc<Node>) {
        self.push_scope();
        for n in root.child_nodes() {
            match n.kind {
                K::FnDecl => self.check_fn(n),
                K::RecordDecl | K::ChoiceDecl | K::ShapeDecl | K::UseDecl => {}
                K::TestDecl => {
                    self.current_ret = Some(Ty::Unit);
                    self.current_rigids = HashSet::new();
                    self.push_scope();
                    if let Some(b) = n.find_node(K::Block) {
                        self.check_block(b);
                    }
                    self.pop_scope();
                    self.current_ret = None;
                }
                _ => {
                    if !self.is_entry_module {
                        self.err(
                            n,
                            "E0246",
                            "only the main file runs top-level code — modules hold definitions (fn, record, choice, shape, test)",
                        );
                    }
                    self.check_stmt(n);
                }
            }
        }
        self.pop_scope();
    }

    fn check_fn(&mut self, n: &Rc<Node>) {
        let name = n
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let Some(sig) = self.fns.get(&name).cloned() else {
            return;
        };
        let mut rigids = Vec::new();
        for p in &sig.params {
            collect_rigids(p, &mut rigids);
        }
        collect_rigids(&sig.ret, &mut rigids);
        self.current_rigids = rigids.into_iter().collect();
        self.current_ret = Some(sig.ret.clone());

        self.push_scope();
        if let Some(pl) = n.find_node(K::ParamList) {
            for (i, p) in pl.nodes_of(K::Param).into_iter().enumerate() {
                let pname = p
                    .find_token(K::Ident)
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                let ty = sig.params.get(i).cloned().unwrap_or(Ty::Any);
                let span = self.spans.of(p);
                // Parameters are never warned as unused, and are immutable.
                self.define(&pname, ty, false, span, true);
            }
        }

        if let Some(b) = n.find_node(K::Block) {
            let trailing = self.check_block(b);
            let ret = sig.ret.clone();
            if self.uni.resolve(&ret) != Ty::Unit {
                if let Some((t, node)) = trailing {
                    if !self.uni.unify(&t, &ret) {
                        let msg = format!(
                            "`{name}` promises to return {}, but this produces {}",
                            self.show(&ret),
                            self.show(&t)
                        );
                        self.err(&node, "E0225", msg);
                    }
                }
            }
        }
        self.pop_scope();
        self.current_ret = None;
        self.current_rigids = HashSet::new();
    }

    /// Checks a block's statements; returns the trailing value (type + node)
    /// if the final statement produces one.
    fn check_block(&mut self, b: &Rc<Node>) -> Option<(Ty, Rc<Node>)> {
        let mut last = None;
        for stmt in b.child_nodes() {
            last = self.check_stmt(stmt).map(|t| (t, stmt.clone()));
        }
        last
    }

    fn check_stmt(&mut self, n: &Rc<Node>) -> Option<Ty> {
        match n.kind {
            K::LetStmt | K::VarStmt => {
                self.check_binding(n);
                None
            }
            K::AssignStmt => {
                self.check_assign(n);
                None
            }
            K::SayStmt => {
                if let Some(e) = expr_children(n).first() {
                    self.ty_of(e);
                }
                None
            }
            K::SpawnStmt => {
                let span = self.spans.of(n);
                self.warn(
                    span,
                    "W0003",
                    "`spawn` runs one line at a time until Milestone M7",
                );
                if let Some(e) = expr_children(n).first() {
                    self.ty_of(e);
                }
                None
            }
            K::ReturnStmt => {
                let t = match expr_children(n).first() {
                    Some(e) => self.ty_of(e),
                    None => Ty::Unit,
                };
                match self.current_ret.clone() {
                    None => self.err(n, "E0226", "`return` only works inside a function"),
                    Some(ret) => {
                        if !self.uni.unify(&t, &ret) {
                            let msg = format!(
                                "this function promises to return {}, but this returns {}",
                                self.show(&ret),
                                self.show(&t)
                            );
                            self.err(n, "E0225", msg);
                        }
                    }
                }
                None
            }
            K::ExprStmt => expr_children(n).first().map(|e| self.ty_of(e)),
            K::IfStmt => self.check_if(n),
            K::WhileStmt => {
                if let Some(c) = expr_children(n).first() {
                    let t = self.ty_of(c);
                    self.require_bool(&t, c, "E0212");
                }
                self.scoped_block(n);
                None
            }
            K::ForStmt => {
                self.check_for(n);
                None
            }
            K::RepeatStmt => {
                if let Some(c) = expr_children(n).first() {
                    let t = self.ty_of(c);
                    if !self.uni.unify(&t, &Ty::Int) {
                        let msg = format!(
                            "`repeat` needs a whole number, but this is {}",
                            self.show(&t)
                        );
                        self.err(c, "E0223", msg);
                    }
                }
                self.scoped_block(n);
                None
            }
            K::DoTogetherStmt => {
                let span = self.spans.of(n);
                self.warn(
                    span,
                    "W0003",
                    "`do together` runs one line at a time until Milestone M7",
                );
                self.scoped_block(n);
                None
            }
            K::MatchStmt => self.check_match(n),
            K::FnDecl | K::RecordDecl | K::ChoiceDecl | K::ShapeDecl | K::TestDecl => {
                self.err(
                    n,
                    "E0242",
                    "functions, records, choices, and shapes live at the top level of a file",
                );
                None
            }
            _ => None,
        }
    }

    fn scoped_block(&mut self, n: &Rc<Node>) {
        self.push_scope();
        if let Some(b) = n.find_node(K::Block) {
            self.check_block(b);
        }
        self.pop_scope();
    }

    fn check_if(&mut self, n: &Rc<Node>) -> Option<Ty> {
        if let Some(c) = expr_children(n).first() {
            let t = self.ty_of(c);
            self.require_bool(&t, c, "E0212");
        }
        self.push_scope();
        let then_val = n.find_node(K::Block).and_then(|b| self.check_block(b));
        self.pop_scope();

        let mut else_val = None;
        if let Some(ec) = n.find_node(K::ElseClause) {
            if let Some(nested) = ec.find_node(K::IfStmt) {
                else_val = self.check_if(nested).map(|t| (t, nested.clone()));
            } else if let Some(b) = ec.find_node(K::Block) {
                self.push_scope();
                else_val = self.check_block(b);
                self.pop_scope();
            }
        }
        match (then_val, else_val) {
            (Some((a, _)), Some((b, _))) if self.uni.unify(&a, &b) => Some(a),
            _ => None,
        }
    }

    fn check_for(&mut self, n: &Rc<Node>) {
        let var = n
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let item_ty = match expr_children(n).first() {
            Some(iter) => {
                let t = self.ty_of(iter);
                match self.uni.resolve(&t) {
                    Ty::List(inner) => *inner,
                    Ty::Range => Ty::Int,
                    Ty::Any => Ty::Any,
                    Ty::Var(_) => {
                        let item = self.uni.fresh();
                        self.uni.unify(&t, &Ty::List(Box::new(item.clone())));
                        item
                    }
                    other => {
                        let msg = format!(
                            "`for` can walk through a List or a range, but this is {}",
                            self.show(&other)
                        );
                        self.err(iter, "E0224", msg);
                        Ty::Any
                    }
                }
            }
            None => Ty::Any,
        };
        self.push_scope();
        let span = self.spans.of(n);
        self.define(&var, item_ty, false, span, false);
        if let Some(b) = n.find_node(K::Block) {
            self.check_block(b);
        }
        self.pop_scope();
    }

    fn check_binding(&mut self, n: &Rc<Node>) {
        let name = n
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let rigids = self.current_rigids.clone();
        let ann = n
            .find_node(K::TypeRef)
            .map(|t| self.resolve_type(t, &rigids));
        let val = match expr_children(n).first() {
            Some(e) => self.ty_of(e),
            None => Ty::Any,
        };
        let ty = match ann {
            Some(a) => {
                if !self.uni.unify(&a, &val) {
                    let msg = format!(
                        "the annotation says {}, but this value is {}",
                        self.show(&a),
                        self.show(&val)
                    );
                    if let Some(e) = expr_children(n).first() {
                        self.err(e, "E0211", msg);
                    } else {
                        self.err(n, "E0211", msg);
                    }
                }
                a
            }
            None => val,
        };
        let span = self.spans.of(n);
        self.define(&name, ty, n.kind == K::VarStmt, span, false);
    }

    fn check_assign(&mut self, n: &Rc<Node>) {
        let exprs = expr_children(n);
        let (Some(lhs), Some(rhs)) = (exprs.first(), exprs.get(1)) else {
            return;
        };
        let compound = n.child_tokens().into_iter().any(|t| {
            matches!(
                t.kind,
                K::PlusAssign | K::MinusAssign | K::StarAssign | K::SlashAssign
            )
        });
        let target_ty = match lhs.kind {
            K::NameRef => {
                let name = lhs
                    .find_token(K::Ident)
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                match self.lookup_local(&name) {
                    Some((ty, mutable)) => {
                        if !mutable {
                            let msg = format!(
                                "`{name}` cannot change — create it with `var` instead of `let` if it should"
                            );
                            self.err(lhs, "E0102", msg);
                        }
                        ty
                    }
                    None => {
                        if self.fns.contains_key(&name) {
                            self.err(
                                lhs,
                                "E0237",
                                format!("`{name}` is a function — only a name, a field, or an index can be assigned to"),
                            );
                        } else {
                            let candidates = self.visible_value_names();
                            let msg = format!(
                                "nothing named `{name}` exists here{}",
                                self.suggestion(&name, &candidates)
                            );
                            self.err(lhs, "E0201", msg);
                        }
                        Ty::Any
                    }
                }
            }
            K::FieldExpr | K::IndexExpr => self.ty_of(lhs),
            _ => {
                self.err(
                    lhs,
                    "E0237",
                    "only a name, a field, or an index position can be assigned to",
                );
                Ty::Any
            }
        };
        let rhs_ty = self.ty_of(rhs);
        if compound {
            self.arith_result(&target_ty, &rhs_ty, n);
        } else if !self.uni.unify(&target_ty, &rhs_ty) {
            let msg = format!(
                "this spot holds {}, but the new value is {}",
                self.show(&target_ty),
                self.show(&rhs_ty)
            );
            self.err(rhs, "E0211", msg);
        }
    }

    fn require_bool(&mut self, t: &Ty, node: &Rc<Node>, code: &'static str) {
        if !self.uni.unify(t, &Ty::Bool) {
            let msg = match code {
                "E0212" => format!(
                    "this condition must be a yes-or-no value (Bool), but it is {}",
                    self.show(t)
                ),
                _ => format!(
                    "`and`, `or`, and `not` need yes-or-no values (Bool), but this is {}",
                    self.show(t)
                ),
            };
            self.err(node, code, msg);
        }
    }

    fn arith_result(&mut self, l: &Ty, r: &Ty, node: &Rc<Node>) -> Ty {
        let lr = self.uni.resolve(l);
        let rr = self.uni.resolve(r);
        match (&lr, &rr) {
            (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => {
                self.err(
                    node,
                    "E0210",
                    "whole numbers (Int) and decimals (Float) can't mix — convert one side with .to_float() or write the number with a decimal point",
                );
                return Ty::Any;
            }
            (Ty::Text, _) | (_, Ty::Text) => {
                self.err(
                    node,
                    "E0214",
                    "math needs numbers — to build text from pieces, use interpolation: \"total: {count}\"",
                );
                return Ty::Any;
            }
            _ => {}
        }
        if !self.uni.unify(l, r) {
            let msg = format!(
                "these can't be combined: {} and {}",
                self.show(l),
                self.show(r)
            );
            self.err(node, "E0211", msg);
            return Ty::Any;
        }
        match self.uni.resolve(l) {
            Ty::Int => Ty::Int,
            Ty::Float => Ty::Float,
            Ty::Any => Ty::Any,
            Ty::Var(_) => {
                self.uni.unify(l, &Ty::Int);
                Ty::Int
            }
            other => {
                let msg = format!("math needs numbers, but this is {}", self.show(&other));
                self.err(node, "E0214", msg);
                Ty::Any
            }
        }
    }

    // ----- expressions -----

    fn ty_of(&mut self, n: &Rc<Node>) -> Ty {
        match n.kind {
            K::Literal => {
                let tok = n
                    .child_tokens()
                    .into_iter()
                    .find(|t| !t.kind.is_trivia())
                    .cloned();
                match tok.as_ref().map(|t| t.kind) {
                    Some(K::IntLit) => Ty::Int,
                    Some(K::FloatLit) => Ty::Float,
                    Some(K::StrLit) => {
                        // Every `{…}` hole is real code: parsed with the real
                        // parser and type-checked in the current scope.
                        if let Some(t) = tok {
                            let text = t.text.clone();
                            self.check_interpolation(n, &text);
                        }
                        Ty::Text
                    }
                    Some(K::TrueKw) | Some(K::FalseKw) => Ty::Bool,
                    _ => Ty::Any,
                }
            }
            K::NameRef => self.name_ref(n),
            K::ParenExpr => match expr_children(n).first() {
                Some(e) => self.ty_of(e),
                None => Ty::Any,
            },
            K::BinaryExpr => self.binary(n),
            K::RangeExpr => {
                for e in expr_children(n) {
                    let t = self.ty_of(e);
                    if !self.uni.unify(&t, &Ty::Int) {
                        let msg = format!(
                            "both ends of a range must be whole numbers, but this is {}",
                            self.show(&t)
                        );
                        self.err(e, "E0229", msg);
                    }
                }
                Ty::Range
            }
            K::UnaryExpr => {
                let inner = match expr_children(n).first() {
                    Some(e) => self.ty_of(e),
                    None => Ty::Any,
                };
                if n.find_token(K::NotKw).is_some() {
                    if !self.uni.unify(&inner, &Ty::Bool) {
                        let msg = format!(
                            "`not` needs a yes-or-no value (Bool), but this is {}",
                            self.show(&inner)
                        );
                        self.err(n, "E0213", msg);
                    }
                    Ty::Bool
                } else {
                    match self.uni.resolve(&inner) {
                        Ty::Int => Ty::Int,
                        Ty::Float => Ty::Float,
                        Ty::Any => Ty::Any,
                        Ty::Var(_) => {
                            self.uni.unify(&inner, &Ty::Int);
                            Ty::Int
                        }
                        other => {
                            let msg =
                                format!("`-` needs a number, but this is {}", self.show(&other));
                            self.err(n, "E0214", msg);
                            Ty::Any
                        }
                    }
                }
            }
            K::CallExpr => self.call(n),
            K::FieldExpr => self.field_access(n),
            K::IndexExpr => self.index(n),
            K::ListExpr => {
                let elem = self.uni.fresh();
                let mut reported = false;
                for e in expr_children(n) {
                    let t = self.ty_of(e);
                    if !self.uni.unify(&elem, &t) && !reported {
                        let msg = format!(
                            "the items in this list are different types: {} and {}",
                            self.show(&elem),
                            self.show(&t)
                        );
                        self.err(e, "E0227", msg);
                        reported = true;
                    }
                }
                Ty::List(Box::new(elem))
            }
            K::MapExpr => {
                let key = self.uni.fresh();
                let val = self.uni.fresh();
                let mut reported = false;
                for entry in n.nodes_of(K::MapEntry) {
                    let kv = expr_children(entry);
                    if let Some(k) = kv.first() {
                        let t = self.ty_of(k);
                        if !self.uni.unify(&key, &t) && !reported {
                            let msg = format!(
                                "map keys must all be one type: {} and {}",
                                self.show(&key),
                                self.show(&t)
                            );
                            self.err(k, "E0228", msg);
                            reported = true;
                        }
                    }
                    if let Some(v) = kv.get(1) {
                        let t = self.ty_of(v);
                        if !self.uni.unify(&val, &t) && !reported {
                            let msg = format!(
                                "map values must all be one type: {} and {}",
                                self.show(&val),
                                self.show(&t)
                            );
                            self.err(v, "E0228", msg);
                            reported = true;
                        }
                    }
                }
                Ty::Map(Box::new(key), Box::new(val))
            }
            K::AskExpr => {
                if let Some(e) = expr_children(n).first() {
                    let t = self.ty_of(e);
                    if !self.uni.unify(&t, &Ty::Text) {
                        let msg =
                            format!("`ask` shows a Text question, but this is {}", self.show(&t));
                        self.err(e, "E0211", msg);
                    }
                }
                Ty::Text
            }
            K::TryExpr => self.try_expr(n),
            _ => Ty::Any,
        }
    }

    fn check_interpolation(&mut self, literal_node: &Rc<Node>, literal_text: &str) {
        use spider_syntax::interpolation::{segments, Segment};
        let span = self.spans.of(literal_node);
        for seg in segments(literal_text) {
            let Segment::Expr(src) = seg else { continue };
            if src.trim().is_empty() {
                self.err_span(span, "E0243", "this text has an empty { } hole");
                continue;
            }
            let fragment = spider_syntax::parse_expr_source(&src);
            if let Some(first) = fragment.diagnostics.first() {
                let msg = format!(
                    "the code inside {{ }} is not a valid expression: {}",
                    first.message
                );
                self.err_span(span, "E0243", msg);
                continue;
            }
            let exprs = expr_children(&fragment.root);
            if let Some(e) = exprs.first() {
                let saved = self.span_override;
                self.span_override = Some(span);
                self.ty_of(e);
                self.span_override = saved;
            }
        }
    }

    fn name_ref(&mut self, n: &Rc<Node>) -> Ty {
        let name = n
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        if let Some((ty, _)) = self.lookup_local(&name) {
            return ty;
        }
        // Zero-arity choice cases are values: `Dot`, `None`.
        if let Some(choice) = self.variant_of.get(&name) {
            let arity_zero = self
                .choices
                .get(choice)
                .and_then(|vs| vs.iter().find(|(v, _)| v == &name))
                .is_some_and(|(_, fields)| fields.is_empty());
            if arity_zero {
                return Ty::Choice(choice.clone());
            }
        }
        if name == "None" {
            let inner = self.uni.fresh();
            return Ty::Maybe(Box::new(inner));
        }
        if let Some(sig) = self.fns.get(&name) {
            return Ty::Fn(sig.params.clone(), Box::new(sig.ret.clone()));
        }
        if self.modules.contains(&name) {
            return Ty::Any;
        }
        let candidates = self.visible_value_names();
        let msg = format!(
            "nothing named `{name}` exists here{}",
            self.suggestion(&name, &candidates)
        );
        self.err(n, "E0201", msg);
        Ty::Any
    }

    fn binary(&mut self, n: &Rc<Node>) -> Ty {
        let exprs = expr_children(n);
        let l = match exprs.first() {
            Some(e) => self.ty_of(e),
            None => Ty::Any,
        };
        let r = match exprs.get(1) {
            Some(e) => self.ty_of(e),
            None => Ty::Any,
        };
        let op = n
            .child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia())
            .map(|t| t.kind);
        match op {
            Some(K::Plus | K::Minus | K::Star | K::Slash | K::Percent) => {
                self.arith_result(&l, &r, n)
            }
            Some(K::EqEq | K::NotEq) => {
                if !self.uni.unify(&l, &r) {
                    let msg = format!(
                        "these two values cannot be compared: {} and {}",
                        self.show(&l),
                        self.show(&r)
                    );
                    self.err(n, "E0215", msg);
                }
                Ty::Bool
            }
            Some(K::Lt | K::LtEq | K::Gt | K::GtEq) => {
                if !self.uni.unify(&l, &r) {
                    let msg = format!(
                        "these two values cannot be compared: {} and {}",
                        self.show(&l),
                        self.show(&r)
                    );
                    self.err(n, "E0215", msg);
                } else {
                    match self.uni.resolve(&l) {
                        Ty::Int | Ty::Float | Ty::Text | Ty::Any => {}
                        Ty::Var(_) => {
                            self.uni.unify(&l, &Ty::Int);
                        }
                        other => {
                            let msg = format!(
                                "{} values have no smaller-or-larger order",
                                self.show(&other)
                            );
                            self.err(n, "E0215", msg);
                        }
                    }
                }
                Ty::Bool
            }
            Some(K::AndKw | K::OrKw) => {
                for (t, e) in [(l, exprs.first()), (r, exprs.get(1))] {
                    if let Some(e) = e {
                        self.require_bool(&t, e, "E0213");
                    }
                }
                Ty::Bool
            }
            _ => Ty::Any,
        }
    }

    fn call(&mut self, n: &Rc<Node>) -> Ty {
        let exprs = expr_children(n);
        let Some(callee) = exprs.first() else {
            return Ty::Any;
        };
        let args: Vec<Rc<Node>> = n
            .find_node(K::ArgList)
            .map(|al| expr_children(al).into_iter().cloned().collect())
            .unwrap_or_default();

        // Named function / constructor call?
        if callee.kind == K::NameRef {
            let name = callee
                .find_token(K::Ident)
                .map(|t| t.text.clone())
                .unwrap_or_default();
            if self
                .scopes
                .iter()
                .rev()
                .all(|s| !s.contains_key(&name))
            {
                // Built-in generic constructors.
                match name.as_str() {
                    "Ok" | "Fail" | "Some" => {
                        if args.len() != 1 {
                            let msg =
                                format!("`{name}` takes 1 argument, but got {}", args.len());
                            self.err(n, "E0216", msg);
                        }
                        let payload = match args.first() {
                            Some(a) => self.ty_of(a),
                            None => Ty::Any,
                        };
                        return match name.as_str() {
                            "Some" => Ty::Maybe(Box::new(payload)),
                            "Ok" => Ty::Outcome(Box::new(payload)),
                            // Fail's payload is the problem, not the success type.
                            _ => {
                                let success = self.uni.fresh();
                                Ty::Outcome(Box::new(success))
                            }
                        };
                    }
                    _ => {}
                }
                if let Some(sig) = self.fns.get(&name).cloned() {
                    return self.check_call_against(&name, &sig, &args, n);
                }
                // The global builtin `expect(actual, expected)`.
                if name == "expect" {
                    if args.len() != 2 {
                        let msg =
                            format!("`expect` takes 2 arguments (actual, expected), but got {}", args.len());
                        self.err(n, "E0216", msg);
                    }
                    let a = args.first().map(|x| self.ty_of(x)).unwrap_or(Ty::Any);
                    let b = args.get(1).map(|x| self.ty_of(x)).unwrap_or(Ty::Any);
                    if !self.uni.unify(&a, &b) {
                        let msg = format!(
                            "`expect` compares two values of the same type, but these are {} and {}",
                            self.show(&a),
                            self.show(&b)
                        );
                        self.err(n, "E0211", msg);
                    }
                    return Ty::Unit;
                }
            }
        }

        // Method call: base.method(args)
        if callee.kind == K::FieldExpr {
            return self.method_call(callee, &args, n);
        }

        let callee_ty = self.ty_of(callee);
        match self.uni.resolve(&callee_ty) {
            Ty::Fn(params, ret) => {
                let sig = FnSig {
                    params,
                    ret: *ret,
                };
                self.check_call_against("this function", &sig, &args, n)
            }
            Ty::Any => {
                for a in &args {
                    self.ty_of(a);
                }
                Ty::Any
            }
            other => {
                for a in &args {
                    self.ty_of(a);
                }
                let msg = format!(
                    "this is {}, and only functions can be called with (…)",
                    self.show(&other)
                );
                self.err(callee, "E0217", msg);
                Ty::Any
            }
        }
    }

    fn check_call_against(
        &mut self,
        name: &str,
        sig: &FnSig,
        args: &[Rc<Node>],
        call: &Rc<Node>,
    ) -> Ty {
        // Instantiate type parameters fresh for this call site.
        let mut rigids = Vec::new();
        for p in &sig.params {
            collect_rigids(p, &mut rigids);
        }
        collect_rigids(&sig.ret, &mut rigids);
        let mut map = HashMap::new();
        for r in rigids {
            let fresh = self.uni.fresh();
            map.insert(r, fresh);
        }
        let params: Vec<Ty> = sig.params.iter().map(|p| subst_rigids(p, &map)).collect();
        let ret = subst_rigids(&sig.ret, &map);

        if args.len() != params.len() {
            let msg = format!(
                "`{name}` takes {} argument(s), but got {}",
                params.len(),
                args.len()
            );
            self.err(call, "E0216", msg);
        }
        for (i, a) in args.iter().enumerate() {
            let at = self.ty_of(a);
            if let Some(p) = params.get(i) {
                if !self.uni.unify(&at, p) {
                    let msg = format!(
                        "argument {} of `{name}` should be {}, but this is {}",
                        i + 1,
                        self.show(p),
                        self.show(&at)
                    );
                    self.err(a, "E0211", msg);
                }
            }
        }
        ret
    }

    fn method_call(&mut self, callee: &Rc<Node>, args: &[Rc<Node>], call: &Rc<Node>) -> Ty {
        let base = match expr_children(callee).first() {
            Some(b) => (*b).clone(),
            None => return Ty::Any,
        };
        let method = callee
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();

        // Module call: typed against the stdlib registry (M4).
        if base.kind == K::NameRef {
            let bname = base
                .find_token(K::Ident)
                .map(|t| t.text.clone())
                .unwrap_or_default();
            if self.modules.contains(&bname)
                && self.scopes.iter().rev().all(|s| !s.contains_key(&bname))
            {
                // A sibling project module?
                if let Some(ex) = self.user_modules.get(&bname).cloned() {
                    match ex.get(&method) {
                        Some((params, ret, true)) => {
                            let sig = FnSig {
                                params: params.clone(),
                                ret: ret.clone(),
                            };
                            let label = format!("{bname}.{method}");
                            return self.check_call_against(&label, &sig, args, call);
                        }
                        Some((_, _, false)) => {
                            let msg = format!(
                                "`{method}` exists in `{bname}`, but it is not public — add `public` to its `fn` to share it"
                            );
                            self.err(callee, "E0306", msg);
                            for a in args {
                                self.ty_of(a);
                            }
                            return Ty::Any;
                        }
                        None => {
                            let names: Vec<String> = ex.keys().cloned().collect();
                            let msg = format!(
                                "the module `{bname}` has no public function named `{method}`{}",
                                self.suggestion(&method, &names)
                            );
                            self.err(callee, "E0306", msg);
                            for a in args {
                                self.ty_of(a);
                            }
                            return Ty::Any;
                        }
                    }
                }
                if let Some(mf) = stdlib::find_module_fn(&bname, &method) {
                    let sig = FnSig {
                        params: mf.params,
                        ret: mf.ret,
                    };
                    let label = format!("{bname}.{method}");
                    return self.check_call_against(&label, &sig, args, call);
                }
                for a in args {
                    self.ty_of(a);
                }
                let msg = if stdlib::module_is_implemented(&bname) {
                    let members = stdlib::module_member_names(&bname);
                    format!(
                        "`{bname}` has no member named `{method}`{}",
                        self.suggestion(&method, &members)
                    )
                } else {
                    format!(
                        "the `{bname}` module has no members in this Spider version yet"
                    )
                };
                self.err(callee, "E0306", msg);
                return Ty::Any;
            }
        }

        let base_ty = self.ty_of(&base);
        let resolved = self.uni.resolve(&base_ty);
        if resolved == Ty::Any {
            for a in args {
                self.ty_of(a);
            }
            return Ty::Any;
        }
        match stdlib::method_sig(&resolved, &method) {
            Some((params, ret)) => {
                let sig = FnSig { params, ret };
                self.check_call_against(&method, &sig, args, call)
            }
            None => {
                for a in args {
                    self.ty_of(a);
                }
                let msg = format!(
                    "{} has no method named `{method}`",
                    self.show(&resolved)
                );
                self.err(callee, "E0221", msg);
                Ty::Any
            }
        }
    }

    fn field_access(&mut self, n: &Rc<Node>) -> Ty {
        let children = expr_children(n);
        let Some(base) = children.first() else {
            return Ty::Any;
        };
        let field = n
            .find_token(K::Ident)
            .map(|t| t.text.clone())
            .unwrap_or_default();

        if base.kind == K::NameRef {
            let bname = base
                .find_token(K::Ident)
                .map(|t| t.text.clone())
                .unwrap_or_default();
            if self.modules.contains(&bname)
                && self.scopes.iter().rev().all(|s| !s.contains_key(&bname))
            {
                if self.user_modules.contains_key(&bname) {
                    let msg = format!(
                        "modules share functions — call one with `{bname}.{field}(…)`"
                    );
                    self.err(n, "E0306", msg);
                    return Ty::Any;
                }
                if let Some(mc) = stdlib::find_module_const(&bname, &field) {
                    return mc.ty;
                }
                let msg = if stdlib::find_module_fn(&bname, &field).is_some() {
                    format!("`{bname}.{field}` is a function — call it with (…)")
                } else if stdlib::module_is_implemented(&bname) {
                    let members = stdlib::module_member_names(&bname);
                    format!(
                        "`{bname}` has no member named `{field}`{}",
                        self.suggestion(&field, &members)
                    )
                } else {
                    format!("the `{bname}` module has no members in this Spider version yet")
                };
                self.err(n, "E0306", msg);
                return Ty::Any;
            }
        }

        let base_ty = self.ty_of(base);
        match self.uni.resolve(&base_ty) {
            Ty::Record(rname) => {
                let fields = self.records.get(&rname).cloned().unwrap_or_default();
                match fields.iter().find(|(f, _)| f == &field) {
                    Some((_, t)) => t.clone(),
                    None => {
                        let names: Vec<String> = fields.iter().map(|(f, _)| f.clone()).collect();
                        let msg = format!(
                            "`{rname}` has no field named `{field}`{}",
                            self.suggestion(&field, &names)
                        );
                        self.err(n, "E0221", msg);
                        Ty::Any
                    }
                }
            }
            Ty::Any | Ty::Var(_) => Ty::Any,
            other => {
                let msg = format!("this is {}, which has no fields", self.show(&other));
                self.err(n, "E0222", msg);
                Ty::Any
            }
        }
    }

    fn index(&mut self, n: &Rc<Node>) -> Ty {
        let exprs = expr_children(n);
        let (Some(base), idx) = (exprs.first(), exprs.get(1)) else {
            return Ty::Any;
        };
        let base_ty = self.ty_of(base);
        let idx_ty = match idx {
            Some(i) => self.ty_of(i),
            None => Ty::Any,
        };
        match self.uni.resolve(&base_ty) {
            Ty::List(item) => {
                if !self.uni.unify(&idx_ty, &Ty::Int) {
                    let msg = format!(
                        "a list is indexed by position (Int), but this is {}",
                        self.show(&idx_ty)
                    );
                    self.err(idx.unwrap_or(base), "E0219", msg);
                }
                *item
            }
            Ty::Map(k, v) => {
                if !self.uni.unify(&idx_ty, &k) {
                    let msg = format!(
                        "this map's keys are {}, but this key is {}",
                        self.show(&k),
                        self.show(&idx_ty)
                    );
                    self.err(idx.unwrap_or(base), "E0220", msg);
                }
                *v
            }
            Ty::Text => {
                self.err(
                    n,
                    "E0218",
                    "Text has no [ ] indexing — human characters are more complicated than positions; use .length() and slices instead",
                );
                Ty::Any
            }
            Ty::Any => Ty::Any,
            Ty::Var(_) => {
                let item = self.uni.fresh();
                self.uni.unify(&base_ty, &Ty::List(Box::new(item.clone())));
                self.uni.unify(&idx_ty, &Ty::Int);
                item
            }
            other => {
                let msg = format!("{} cannot be indexed with [ ]", self.show(&other));
                self.err(n, "E0218", msg);
                Ty::Any
            }
        }
    }

    fn try_expr(&mut self, n: &Rc<Node>) -> Ty {
        let exprs = expr_children(n);
        let inner_ty = match exprs.first() {
            Some(e) => self.ty_of(e),
            None => Ty::Any,
        };
        let fallback = exprs.get(1);
        let payload = match self.uni.resolve(&inner_ty) {
            Ty::Outcome(t) | Ty::Maybe(t) => *t,
            Ty::Any => Ty::Any,
            other => {
                let msg = format!(
                    "`try` needs something that can fail (Outcome or Maybe), but this is {}",
                    self.show(&other)
                );
                let at = exprs.first().map_or(n, |e| *e).clone();
                self.err(&at, "E0235", msg);
                Ty::Any
            }
        };
        match fallback {
            Some(fb) => {
                let fb_ty = self.ty_of(fb);
                if !self.uni.unify(&payload, &fb_ty) {
                    let msg = format!(
                        "the fallback after `else` is {}, but the tried value gives {}",
                        self.show(&fb_ty),
                        self.show(&payload)
                    );
                    self.err(fb, "E0211", msg);
                }
                payload
            }
            None => {
                let ret_is_outcome = matches!(
                    self.current_ret.as_ref().map(|r| self.uni.resolve(r)),
                    Some(Ty::Outcome(_))
                );
                if !ret_is_outcome && self.uni.resolve(&inner_ty) != Ty::Any {
                    self.err(
                        n,
                        "E0236",
                        "a bare `try` passes failure upward — add `else fallback`, or make this function return `Outcome of …`",
                    );
                }
                payload
            }
        }
    }

    // ----- match -----

    fn check_match(&mut self, n: &Rc<Node>) -> Option<Ty> {
        let scrut_ty = match expr_children(n).first() {
            Some(e) => self.ty_of(e),
            None => Ty::Any,
        };
        let Some(block) = n.find_node(K::Block) else {
            return None;
        };

        let mut covered: Vec<String> = Vec::new();
        let mut catchall = false;
        let mut arm_vals: Vec<(Ty, bool, Rc<Node>)> = Vec::new();

        for arm in block.nodes_of(K::MatchArm) {
            self.push_scope();
            if catchall {
                self.err(
                    arm,
                    "E0234",
                    "an earlier case already catches everything, so this line can never run",
                );
            }
            if let Some(pat) = arm.find_node(K::Pattern) {
                if let Some(tag) = self.bind_pattern(pat, &scrut_ty, &mut catchall) {
                    if covered.contains(&tag) {
                        let msg = format!("the case `{tag}` is already handled above");
                        self.err(pat, "E0234", msg);
                    }
                    covered.push(tag);
                }
            }
            let is_say = arm.find_token(K::SayKw).is_some();
            let val = match expr_children(arm).first() {
                Some(e) => self.ty_of(e),
                None => Ty::Any,
            };
            arm_vals.push((val, is_say, arm.clone()));
            self.pop_scope();
        }

        // Exhaustiveness.
        let all: Option<Vec<String>> = match self.uni.resolve(&scrut_ty) {
            Ty::Choice(name) => self
                .choices
                .get(&name)
                .map(|vs| vs.iter().map(|(v, _)| v.clone()).collect()),
            Ty::Maybe(_) => Some(vec!["Some".into(), "None".into()]),
            Ty::Outcome(_) => Some(vec!["Ok".into(), "Fail".into()]),
            Ty::Bool => Some(vec!["true".into(), "false".into()]),
            Ty::Int | Ty::Float | Ty::Text => Some(Vec::new()),
            _ => None,
        };
        if let Some(all) = all {
            if !catchall {
                let missing: Vec<String> = if all.is_empty() {
                    vec!["a catch-all name".into()]
                } else {
                    all.into_iter().filter(|v| !covered.contains(v)).collect()
                };
                if !missing.is_empty() {
                    let msg = format!(
                        "this match does not cover: {} — add the missing case(s) or a final catch-all name",
                        missing.join(", ")
                    );
                    self.err(n, "E0230", msg);
                }
            }
        }

        // Result type: statement-matches (any `say` arm) produce no value.
        if arm_vals.iter().any(|(_, is_say, _)| *is_say) {
            return None;
        }
        let mut result: Option<Ty> = None;
        for (t, _, arm) in &arm_vals {
            match &result {
                None => result = Some(t.clone()),
                Some(prev) => {
                    if !self.uni.unify(prev, t) {
                        let msg = format!(
                            "the cases of this match produce different types: {} and {}",
                            self.show(prev),
                            self.show(t)
                        );
                        self.err(arm, "E0240", msg);
                        return None;
                    }
                }
            }
        }
        result
    }

    /// Binds pattern names; returns the coverage tag (variant name, literal
    /// text) or None when nothing coverable was found.
    fn bind_pattern(
        &mut self,
        pat: &Rc<Node>,
        expected: &Ty,
        catchall: &mut bool,
    ) -> Option<String> {
        let first = pat
            .child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia())
            .cloned();
        let subs: Vec<Rc<Node>> = pat.nodes_of(K::Pattern).into_iter().cloned().collect();
        let has_parens = pat.find_token(K::LParen).is_some();
        let tok = first?;

        match tok.kind {
            K::IntLit | K::FloatLit | K::StrLit | K::TrueKw | K::FalseKw => {
                let lit_ty = match tok.kind {
                    K::IntLit => Ty::Int,
                    K::FloatLit => Ty::Float,
                    K::StrLit => Ty::Text,
                    _ => Ty::Bool,
                };
                if !self.uni.unify(&lit_ty, expected) {
                    let msg = format!(
                        "this pattern is {}, but the matched value is {}",
                        self.show(&lit_ty),
                        self.show(expected)
                    );
                    self.err(pat, "E0211", msg);
                }
                Some(tok.text.clone())
            }
            K::Ident => {
                let name = tok.text.clone();
                let resolved = self.uni.resolve(expected);
                // Built-in Maybe/Outcome cases.
                let builtin: Option<(&str, Vec<Ty>)> = match (&resolved, name.as_str()) {
                    (Ty::Maybe(t), "Some") => Some(("Some", vec![(**t).clone()])),
                    (Ty::Maybe(_), "None") => Some(("None", vec![])),
                    (Ty::Outcome(t), "Ok") => Some(("Ok", vec![(**t).clone()])),
                    (Ty::Outcome(_), "Fail") => Some(("Fail", vec![Ty::Any])),
                    _ => None,
                };
                if let Some((vname, fields)) = builtin {
                    self.check_pattern_fields(pat, &name, &fields, &subs, has_parens);
                    return Some(vname.to_string());
                }
                let looks_like_case = name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase());
                if let Ty::Maybe(_) | Ty::Outcome(_) = &resolved {
                    if has_parens || looks_like_case {
                        let names: Vec<String> = match &resolved {
                            Ty::Maybe(_) => vec!["Some".into(), "None".into()],
                            _ => vec!["Ok".into(), "Fail".into()],
                        };
                        let msg = format!(
                            "{} has no case named `{name}`{}",
                            self.show(&resolved),
                            self.suggestion(&name, &names)
                        );
                        self.err(pat, "E0231", msg);
                        return None;
                    }
                }
                if let Ty::Choice(cname) = &resolved {
                    let variants = self.choices.get(cname).cloned().unwrap_or_default();
                    if let Some((_, fields)) = variants.iter().find(|(v, _)| v == &name) {
                        let fields = fields.clone();
                        self.check_pattern_fields(pat, &name, &fields, &subs, has_parens);
                        return Some(name);
                    }
                    // Spider convention: cases are CapWords, bindings are
                    // snake_case. A capitalized unknown here is a misspelled
                    // case, not a catch-all.
                    if has_parens || looks_like_case {
                        let names: Vec<String> =
                            variants.iter().map(|(v, _)| v.clone()).collect();
                        let msg = format!(
                            "`{cname}` has no case named `{name}`{}",
                            self.suggestion(&name, &names)
                        );
                        self.err(pat, "E0231", msg);
                        return None;
                    }
                }
                if has_parens {
                    let msg = format!(
                        "this pattern unpacks parts, but the matched value is {}, not a choice",
                        self.show(&resolved)
                    );
                    self.err(pat, "E0233", msg);
                    return None;
                }
                // Catch-all binding.
                let span = self.spans.of(pat);
                self.define(&name, expected.clone(), false, span, false);
                *catchall = true;
                None
            }
            _ => None,
        }
    }

    fn check_pattern_fields(
        &mut self,
        pat: &Rc<Node>,
        name: &str,
        fields: &[Ty],
        subs: &[Rc<Node>],
        has_parens: bool,
    ) {
        if fields.is_empty() {
            if has_parens {
                let msg = format!("the case `{name}` carries no parts, so it takes no (…)");
                self.err(pat, "E0232", msg);
            }
            return;
        }
        if subs.len() != fields.len() {
            let msg = format!(
                "the case `{name}` carries {} part(s), but this pattern has {}",
                fields.len(),
                subs.len()
            );
            self.err(pat, "E0232", msg);
        }
        for (sub, fty) in subs.iter().zip(fields.iter()) {
            let mut unused_catchall = false;
            self.bind_pattern(sub, fty, &mut unused_catchall);
        }
    }
}

fn expr_children(n: &Node) -> Vec<&Rc<Node>> {
    n.child_nodes()
        .into_iter()
        .filter(|c| c.kind.is_expr())
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
