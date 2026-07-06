//! CST → Silk bytecode.
//!
//! Register-based (ADR-005): every function gets a flat frame of virtual
//! registers, allocated at compile time; the frame size is the high-water
//! mark. Register 0..n hold the parameters. The compiler assumes the checker
//! has passed — internal inconsistencies return `Err(String)` (a toolchain
//! bug report), never a panic.

use crate::value::{RecordShape, Value};
use spider_syntax::interpolation::{segments, Segment};
use spider_syntax::{Node, Parse, SyntaxKind as K};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Instr {
    LoadConst(u16, u32),
    LoadUnit(u16),
    Move(u16, u16),
    LoadGlobal(u16, u16),
    StoreGlobal(u16, u16),
    Bin(BinOp, u16, u16, u16),
    Neg(u16, u16),
    Not(u16, u16),
    Jump(u32),
    JumpIfFalse(u16, u32),
    JumpIfTrue(u16, u32),
    Call(u16, u32, Vec<u16>),
    CallValue(u16, u16, Vec<u16>),
    CallMethod(u16, u16, u32, Vec<u16>),
    CallModule(u16, u8, u32, Vec<u16>),
    ModuleConst(u16, u8, u32),
    MakeList(u16, Vec<u16>),
    MakeMap(u16, Vec<(u16, u16)>),
    MakeRange(u16, u16, u16),
    Index(u16, u16, u16),
    IndexSet(u16, u16, u16),
    GetField(u16, u16, u32),
    SetField(u16, u32, u16),
    MakeRecord(u16, u32, Vec<u16>),
    MakeVariant(u16, u32, Vec<u16>),
    TestTag(u16, u16, u32),
    GetVariantField(u16, u16, u16),
    TryUnwrap(u16, u16, u32),
    NoneToFail(u16),
    IterNew(u16, u16),
    IterNext(u16, u16, u32),
    Concat(u16, Vec<u16>),
    Say(u16),
    Ask(u16, u16),
    Ret(u16),
    RetUnit,
}

#[derive(Debug, Clone)]
pub struct FnProto {
    pub name: String,
    pub n_params: u16,
    pub n_regs: u16,
    pub code: Vec<Instr>,
    pub consts: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub protos: Vec<FnProto>,
    pub entry: u32,
    pub main: Option<u32>,
    pub n_globals: u16,
    /// `test "name"` blocks, in source order — run by `spider test`.
    pub tests: Vec<(String, u32)>,
}

pub const MODULE_MATH: u8 = 0;
pub const MODULE_RANDOM: u8 = 1;
pub const MODULE_FILES: u8 = 2;
/// Global builtins (`expect`) route through the module mechanism.
pub const MODULE_BUILTIN: u8 = 250;
pub const MODULE_UNKNOWN: u8 = 255;

/// Methods that mutate their receiver; the compiler writes the receiver
/// back through its l-value path so value semantics hold.
const MUTATING_METHODS: &[&str] = &["push"];

/// One project module for `compile_project`.
pub struct ModuleSrc<'a> {
    pub name: String,
    pub parse: &'a Parse,
    /// use-alias -> target module name for imports resolved to files.
    pub imports: HashMap<String, String>,
}

pub fn compile(
    parse: &Parse,
    preset_globals: Option<&HashMap<String, u16>>,
) -> Result<Program, String> {
    let single = [ModuleSrc {
        name: "main".into(),
        parse,
        imports: HashMap::new(),
    }];
    compile_project(&single, 0, preset_globals)
}

pub fn compile_project(
    mods: &[ModuleSrc],
    entry: usize,
    preset_globals: Option<&HashMap<String, u16>>,
) -> Result<Program, String> {
    let mut c = Compiler {
        fn_ids: HashMap::new(),
        fn_decls: Vec::new(),
        records: HashMap::new(),
        variants: HashMap::new(),
        modules: HashMap::new(),
        module_imports: HashMap::new(),
        current_module: String::new(),
        globals: preset_globals.cloned().unwrap_or_default(),
        protos: Vec::new(),
        tests: Vec::new(),
    };
    c.reserve_proto("<script>", 0);
    for m in mods {
        c.current_module = m.name.clone();
        c.module_imports.insert(m.name.clone(), m.imports.clone());
        c.collect(&m.parse.root)?;
    }
    c.current_module = mods[entry].name.clone();
    c.compile_all(&mods[entry].parse.root)?;
    let main_key = format!("{}::main", mods[entry].name);
    let main = c
        .fn_ids
        .get(&main_key)
        .copied()
        .filter(|&idx| c.protos.get(idx as usize).is_some_and(|p| p.n_params == 0));
    let n_globals = c
        .globals
        .values()
        .copied()
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    Ok(Program {
        protos: c.protos,
        entry: 0,
        main,
        n_globals,
        tests: c.tests,
    })
}

pub fn globals_of(parse: &Parse, preset: &HashMap<String, u16>) -> HashMap<String, u16> {
    // Recomputes the global map exactly as `compile` allocates it, so a REPL
    // session can persist name -> slot across entries.
    let mut map = preset.clone();
    let mut next = map.values().copied().max().map(|m| m + 1).unwrap_or(0);
    for stmt in parse.root.child_nodes() {
        if matches!(stmt.kind, K::LetStmt | K::VarStmt) {
            if let Some(name) = stmt.find_token(K::Ident) {
                map.entry(name.text.clone()).or_insert_with(|| {
                    let g = next;
                    next += 1;
                    g
                });
            }
        }
    }
    map
}

struct RecordInfo {
    shape: Rc<RecordShape>,
    ctor: u32,
}

struct VariantInfo {
    arity: usize,
    ctor: Option<u32>,
}

struct Compiler {
    /// Function ids, qualified `module::name`. Constructors are global.
    fn_ids: HashMap<String, u32>,
    fn_decls: Vec<(u32, Rc<Node>, String)>,
    records: HashMap<String, RecordInfo>,
    variants: HashMap<String, VariantInfo>,
    modules: HashMap<String, u8>,
    /// module name -> (use-alias -> target module name)
    module_imports: HashMap<String, HashMap<String, String>>,
    current_module: String,
    globals: HashMap<String, u16>,
    protos: Vec<FnProto>,
    tests: Vec<(String, u32)>,
}

impl Compiler {
    fn qualify(&self, name: &str) -> String {
        format!("{}::{name}", self.current_module)
    }

    fn user_import(&self, alias: &str) -> Option<&String> {
        self.module_imports
            .get(&self.current_module)
            .and_then(|m| m.get(alias))
    }

    fn reserve_proto(&mut self, name: &str, n_params: u16) -> u32 {
        let idx = self.protos.len() as u32;
        self.protos.push(FnProto {
            name: name.to_string(),
            n_params,
            n_regs: n_params,
            code: Vec::new(),
            consts: Vec::new(),
        });
        idx
    }

    fn collect(&mut self, root: &Rc<Node>) -> Result<(), String> {
        for n in root.child_nodes() {
            match n.kind {
                K::UseDecl => {
                    let segs: Vec<String> = n
                        .child_tokens()
                        .into_iter()
                        .filter(|t| t.kind == K::Ident)
                        .map(|t| t.text.clone())
                        .collect();
                    if let Some(last) = segs.last() {
                        // Imports the loader resolved to project files are
                        // routed by module_imports, not the stdlib table.
                        let is_user = self
                            .module_imports
                            .get(&self.current_module)
                            .is_some_and(|m| m.contains_key(last));
                        if !is_user {
                            let id = match segs.first().map(|s| s.as_str()) {
                                Some("math") => MODULE_MATH,
                                Some("random") => MODULE_RANDOM,
                                Some("files") => MODULE_FILES,
                                _ => MODULE_UNKNOWN,
                            };
                            self.modules.insert(last.clone(), id);
                        }
                    }
                }
                K::FnDecl => {
                    let name = ident_of(n).ok_or("function without a name")?;
                    let params = n
                        .find_node(K::ParamList)
                        .map(|pl| pl.nodes_of(K::Param).len())
                        .unwrap_or(0) as u16;
                    let idx = self.reserve_proto(&name, params);
                    self.fn_ids.insert(self.qualify(&name), idx);
                    self.fn_decls
                        .push((idx, n.clone(), self.current_module.clone()));
                }
                K::TestDecl => {
                    let raw = n
                        .find_token(K::StrLit)
                        .map(|t| t.text.clone())
                        .unwrap_or_default();
                    let mut name = spider_syntax::interpolation::plain_text(&raw);
                    if self.current_module != "main" {
                        name = format!("{}: {name}", self.current_module);
                    }
                    let idx = self.reserve_proto(&format!("test {name}"), 0);
                    self.tests.push((name, idx));
                    self.fn_decls
                        .push((idx, n.clone(), self.current_module.clone()));
                }
                K::RecordDecl => {
                    let name = ident_of(n).ok_or("record without a name")?;
                    let mut fields = Vec::new();
                    if let Some(b) = n.find_node(K::Block) {
                        for f in b.nodes_of(K::FieldDecl) {
                            fields.push(ident_of(f).unwrap_or_default());
                        }
                    }
                    let shape = Rc::new(RecordShape {
                        name: name.clone(),
                        fields,
                    });
                    let ctor = self.reserve_proto(&name, shape.fields.len() as u16);
                    self.fn_ids.insert(name.clone(), ctor);
                    self.records.insert(name, RecordInfo { shape, ctor });
                }
                K::ChoiceDecl => {
                    if let Some(b) = n.find_node(K::Block) {
                        for v in b.nodes_of(K::VariantDecl) {
                            let vname = ident_of(v).unwrap_or_default();
                            let arity = v.nodes_of(K::Param).len();
                            let ctor = if arity > 0 {
                                let idx = self.reserve_proto(&vname, arity as u16);
                                self.fn_ids.insert(vname.clone(), idx);
                                Some(idx)
                            } else {
                                None
                            };
                            self.variants.insert(vname, VariantInfo { arity, ctor });
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn compile_all(&mut self, root: &Rc<Node>) -> Result<(), String> {
        // Constructor bodies. A record constructor's shape rides in its
        // const table as a fieldless Record value (MakeRecord's u32 is a
        // const index; the VM reads the shape from it).
        for info in self.records.values() {
            let ctor = info.ctor as usize;
            let n = info.shape.fields.len() as u16;
            let proto = &mut self.protos[ctor];
            let cidx = proto.consts.len() as u32;
            proto
                .consts
                .push(Value::Record(Rc::new(crate::value::RecordVal {
                    shape: info.shape.clone(),
                    fields: Vec::new(),
                })));
            proto.n_regs = n + 1;
            proto
                .code
                .push(Instr::MakeRecord(n, cidx, (0..n).collect()));
            proto.code.push(Instr::Ret(n));
        }
        let variant_ctors: Vec<(u32, String, usize)> = self
            .variants
            .iter()
            .filter_map(|(name, v)| v.ctor.map(|c| (c, name.clone(), v.arity)))
            .collect();
        for (ctor, name, arity) in variant_ctors {
            let n = arity as u16;
            let proto = &mut self.protos[ctor as usize];
            let cidx = proto.consts.len() as u32;
            proto.consts.push(Value::text(name));
            proto.n_regs = n + 1;
            proto
                .code
                .push(Instr::MakeVariant(n, cidx, (0..n).collect()));
            proto.code.push(Instr::Ret(n));
        }

        // The script entry: all top-level statements.
        let mut fb = Fb::new(0, true);
        let mut last = None;
        for stmt in root.child_nodes() {
            match stmt.kind {
                K::FnDecl
                | K::RecordDecl
                | K::ChoiceDecl
                | K::ShapeDecl
                | K::UseDecl
                | K::TestDecl => {}
                _ => last = self.stmt(&mut fb, stmt)?,
            }
        }
        match last {
            Some(r) => fb.code.push(Instr::Ret(r)),
            None => fb.code.push(Instr::RetUnit),
        }
        fb.store(&mut self.protos[0]);

        // Function bodies (from every module of the project).
        let decls = std::mem::take(&mut self.fn_decls);
        for (idx, decl, module) in &decls {
            self.current_module = module.clone();
            let n_params = self.protos[*idx as usize].n_params;
            let mut fb = Fb::new(n_params, false);
            if let Some(pl) = decl.find_node(K::ParamList) {
                for (i, p) in pl.nodes_of(K::Param).into_iter().enumerate() {
                    if let Some(name) = ident_of(p) {
                        fb.bind(&name, i as u16);
                    }
                }
            }
            let mut last = None;
            if let Some(b) = decl.find_node(K::Block) {
                for stmt in b.child_nodes() {
                    last = self.stmt(&mut fb, stmt)?;
                }
            }
            match last {
                Some(r) => fb.code.push(Instr::Ret(r)),
                None => fb.code.push(Instr::RetUnit),
            }
            fb.store(&mut self.protos[*idx as usize]);
        }
        self.fn_decls = decls;
        Ok(())
    }

    // ----- statements -----

    fn stmt(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<Option<u16>, String> {
        match n.kind {
            K::LetStmt | K::VarStmt => {
                let name = ident_of(n).ok_or("binding without a name")?;
                let val = match exprs(n).first() {
                    Some(e) => self.expr(fb, e)?,
                    None => {
                        let r = fb.reg();
                        fb.code.push(Instr::LoadUnit(r));
                        r
                    }
                };
                if fb.is_entry && fb.scopes.len() == 1 {
                    let g = self.global_slot(&name);
                    fb.code.push(Instr::StoreGlobal(g, val));
                } else {
                    let r = fb.reg();
                    fb.code.push(Instr::Move(r, val));
                    fb.bind(&name, r);
                }
                Ok(None)
            }
            K::AssignStmt => {
                self.assign(fb, n)?;
                Ok(None)
            }
            K::SayStmt => {
                if let Some(e) = exprs(n).first() {
                    let r = self.expr(fb, e)?;
                    fb.code.push(Instr::Say(r));
                }
                Ok(None)
            }
            K::SpawnStmt => {
                // Sequential until M7 (W0003).
                if let Some(e) = exprs(n).first() {
                    self.expr(fb, e)?;
                }
                Ok(None)
            }
            K::ReturnStmt => {
                match exprs(n).first() {
                    Some(e) => {
                        let r = self.expr(fb, e)?;
                        fb.code.push(Instr::Ret(r));
                    }
                    None => fb.code.push(Instr::RetUnit),
                }
                Ok(None)
            }
            K::ExprStmt => match exprs(n).first() {
                Some(e) => Ok(Some(self.expr(fb, e)?)),
                None => Ok(None),
            },
            K::IfStmt => Ok(Some(self.if_stmt(fb, n)?)),
            K::WhileStmt => {
                let start = fb.code.len() as u32;
                let cond = match exprs(n).first() {
                    Some(c) => self.expr(fb, c)?,
                    None => return Err("while without a condition".into()),
                };
                let exit = fb.emit_patch(Instr::JumpIfFalse(cond, 0));
                fb.push_scope();
                if let Some(b) = n.find_node(K::Block) {
                    for s in b.child_nodes() {
                        self.stmt(fb, s)?;
                    }
                }
                fb.pop_scope();
                fb.code.push(Instr::Jump(start));
                fb.patch(exit);
                Ok(None)
            }
            K::ForStmt => {
                let iter_src = match exprs(n).first() {
                    Some(e) => self.expr(fb, e)?,
                    None => return Err("for without a collection".into()),
                };
                let it = fb.reg();
                fb.code.push(Instr::IterNew(it, iter_src));
                let item = fb.reg();
                let start = fb.code.len() as u32;
                let exit = fb.emit_patch(Instr::IterNext(item, it, 0));
                fb.push_scope();
                if let Some(name) = ident_of(n) {
                    fb.bind(&name, item);
                }
                if let Some(b) = n.find_node(K::Block) {
                    for s in b.child_nodes() {
                        self.stmt(fb, s)?;
                    }
                }
                fb.pop_scope();
                fb.code.push(Instr::Jump(start));
                fb.patch(exit);
                Ok(None)
            }
            K::RepeatStmt => {
                let count = match exprs(n).first() {
                    Some(e) => self.expr(fb, e)?,
                    None => return Err("repeat without a count".into()),
                };
                let i = fb.reg();
                let zero = fb.const_val(Value::Int(0));
                fb.code.push(Instr::LoadConst(i, zero));
                let one_reg = fb.reg();
                let one = fb.const_val(Value::Int(1));
                fb.code.push(Instr::LoadConst(one_reg, one));
                let start = fb.code.len() as u32;
                let t = fb.reg();
                fb.code.push(Instr::Bin(BinOp::Lt, t, i, count));
                let exit = fb.emit_patch(Instr::JumpIfFalse(t, 0));
                fb.push_scope();
                if let Some(b) = n.find_node(K::Block) {
                    for s in b.child_nodes() {
                        self.stmt(fb, s)?;
                    }
                }
                fb.pop_scope();
                fb.code.push(Instr::Bin(BinOp::Add, i, i, one_reg));
                fb.code.push(Instr::Jump(start));
                fb.patch(exit);
                Ok(None)
            }
            K::DoTogetherStmt => {
                fb.push_scope();
                if let Some(b) = n.find_node(K::Block) {
                    for s in b.child_nodes() {
                        self.stmt(fb, s)?;
                    }
                }
                fb.pop_scope();
                Ok(None)
            }
            K::MatchStmt => Ok(Some(self.match_stmt(fb, n)?)),
            _ => Ok(None),
        }
    }

    fn if_stmt(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let dst = fb.reg();
        self.if_into(fb, n, dst)?;
        Ok(dst)
    }

    fn if_into(&mut self, fb: &mut Fb, n: &Rc<Node>, dst: u16) -> Result<(), String> {
        let cond = match exprs(n).first() {
            Some(c) => self.expr(fb, c)?,
            None => return Err("if without a condition".into()),
        };
        let to_else = fb.emit_patch(Instr::JumpIfFalse(cond, 0));
        fb.push_scope();
        let mut then_val = None;
        if let Some(b) = n.find_node(K::Block) {
            for s in b.child_nodes() {
                then_val = self.stmt(fb, s)?;
            }
        }
        fb.pop_scope();
        match then_val {
            Some(v) => fb.code.push(Instr::Move(dst, v)),
            None => fb.code.push(Instr::LoadUnit(dst)),
        }
        let to_end = fb.emit_patch(Instr::Jump(0));
        fb.patch(to_else);
        if let Some(ec) = n.find_node(K::ElseClause) {
            if let Some(nested) = ec.find_node(K::IfStmt) {
                self.if_into(fb, nested, dst)?;
            } else {
                fb.push_scope();
                let mut else_val = None;
                if let Some(b) = ec.find_node(K::Block) {
                    for s in b.child_nodes() {
                        else_val = self.stmt(fb, s)?;
                    }
                }
                fb.pop_scope();
                match else_val {
                    Some(v) => fb.code.push(Instr::Move(dst, v)),
                    None => fb.code.push(Instr::LoadUnit(dst)),
                }
            }
        } else {
            fb.code.push(Instr::LoadUnit(dst));
        }
        fb.patch(to_end);
        Ok(())
    }

    fn match_stmt(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let scrut = match exprs(n).first() {
            Some(e) => self.expr(fb, e)?,
            None => return Err("match without a value".into()),
        };
        let dst = fb.reg();
        let mut end_jumps = Vec::new();
        if let Some(block) = n.find_node(K::Block) {
            for arm in block.nodes_of(K::MatchArm) {
                let mut fails = Vec::new();
                fb.push_scope();
                if let Some(pat) = arm.find_node(K::Pattern) {
                    self.pattern(fb, pat, scrut, &mut fails)?;
                }
                let is_say = arm.find_token(K::SayKw).is_some();
                match exprs(arm).first() {
                    Some(e) => {
                        let v = self.expr(fb, e)?;
                        if is_say {
                            fb.code.push(Instr::Say(v));
                            fb.code.push(Instr::LoadUnit(dst));
                        } else {
                            fb.code.push(Instr::Move(dst, v));
                        }
                    }
                    None => fb.code.push(Instr::LoadUnit(dst)),
                }
                fb.pop_scope();
                end_jumps.push(fb.emit_patch(Instr::Jump(0)));
                for f in fails {
                    fb.patch(f);
                }
            }
        }
        // Exhaustiveness is checker-guaranteed; defensive default.
        fb.code.push(Instr::LoadUnit(dst));
        for j in end_jumps {
            fb.patch(j);
        }
        Ok(dst)
    }

    fn pattern(
        &mut self,
        fb: &mut Fb,
        pat: &Rc<Node>,
        val: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), String> {
        let tok = pat
            .child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia())
            .cloned();
        let Some(tok) = tok else { return Ok(()) };
        match tok.kind {
            K::IntLit | K::FloatLit | K::StrLit | K::TrueKw | K::FalseKw => {
                let cv = if tok.kind == K::StrLit {
                    // Text patterns are plain text (escapes apply, no holes).
                    Value::text(spider_syntax::interpolation::plain_text(&tok.text))
                } else {
                    literal_value(&tok.kind, &tok.text)?
                };
                let cidx = fb.const_val(cv);
                let c = fb.reg();
                fb.code.push(Instr::LoadConst(c, cidx));
                let t = fb.reg();
                fb.code.push(Instr::Bin(BinOp::Eq, t, val, c));
                fails.push(fb.emit_patch(Instr::JumpIfFalse(t, 0)));
            }
            K::Ident => {
                let name = tok.text.clone();
                let is_tag = matches!(name.as_str(), "Some" | "None" | "Ok" | "Fail")
                    || self.variants.contains_key(&name);
                if is_tag {
                    let cidx = fb.const_val(Value::text(name));
                    let t = fb.reg();
                    fb.code.push(Instr::TestTag(t, val, cidx));
                    fails.push(fb.emit_patch(Instr::JumpIfFalse(t, 0)));
                    for (i, sub) in pat.nodes_of(K::Pattern).into_iter().enumerate() {
                        let f = fb.reg();
                        fb.code.push(Instr::GetVariantField(f, val, i as u16));
                        self.pattern(fb, sub, f, fails)?;
                    }
                } else {
                    let r = fb.reg();
                    fb.code.push(Instr::Move(r, val));
                    fb.bind(&name, r);
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ----- assignment and l-value paths -----

    fn assign(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<(), String> {
        let parts = exprs(n);
        let (Some(lhs), Some(rhs)) = (parts.first(), parts.get(1)) else {
            return Err("assignment missing a side".into());
        };
        let op = n.child_tokens().into_iter().find_map(|t| match t.kind {
            K::PlusAssign => Some(BinOp::Add),
            K::MinusAssign => Some(BinOp::Sub),
            K::StarAssign => Some(BinOp::Mul),
            K::SlashAssign => Some(BinOp::Div),
            _ => None,
        });
        match op {
            None => {
                let val = self.expr(fb, rhs)?;
                self.assign_to(fb, lhs, val)
            }
            Some(op) => {
                let (old, wb) = self.read_place(fb, lhs)?;
                let rv = self.expr(fb, rhs)?;
                let new = fb.reg();
                fb.code.push(Instr::Bin(op, new, old, rv));
                self.apply_writeback(fb, new, wb);
                Ok(())
            }
        }
    }

    fn assign_to(&mut self, fb: &mut Fb, lv: &Rc<Node>, val: u16) -> Result<(), String> {
        match lv.kind {
            K::NameRef => {
                let name = ident_of(lv).ok_or("assignment to a nameless place")?;
                if let Some(r) = fb.lookup(&name) {
                    fb.code.push(Instr::Move(r, val));
                } else if fb.is_entry && self.globals.contains_key(&name) {
                    let g = self.globals[&name];
                    fb.code.push(Instr::StoreGlobal(g, val));
                } else {
                    return Err(format!("assignment to unknown name `{name}`"));
                }
                Ok(())
            }
            K::IndexExpr => {
                let parts = exprs(lv);
                let (Some(base), Some(idx)) = (parts.first(), parts.get(1)) else {
                    return Err("index assignment missing a part".into());
                };
                let (br, wb) = self.read_place(fb, base)?;
                let ir = self.expr(fb, idx)?;
                fb.code.push(Instr::IndexSet(br, ir, val));
                self.apply_writeback(fb, br, wb);
                Ok(())
            }
            K::FieldExpr => {
                let parts = exprs(lv);
                let Some(base) = parts.first() else {
                    return Err("field assignment missing its base".into());
                };
                let fname = ident_of(lv).ok_or("field assignment without a field")?;
                let (br, wb) = self.read_place(fb, base)?;
                let cidx = fb.const_val(Value::text(fname));
                fb.code.push(Instr::SetField(br, cidx, val));
                self.apply_writeback(fb, br, wb);
                Ok(())
            }
            _ => Err("only a name, field, or index can be assigned to".into()),
        }
    }

    /// Reads an l-value path into a register and returns how to write the
    /// (possibly mutated) register back through the path.
    fn read_place(&mut self, fb: &mut Fb, lv: &Rc<Node>) -> Result<(u16, Writeback), String> {
        match lv.kind {
            K::NameRef => {
                let name = ident_of(lv).unwrap_or_default();
                if let Some(r) = fb.lookup(&name) {
                    return Ok((r, Writeback::Slot(r)));
                }
                if fb.is_entry {
                    if let Some(&g) = self.globals.get(&name) {
                        let t = fb.reg();
                        fb.code.push(Instr::LoadGlobal(t, g));
                        return Ok((t, Writeback::Global(g)));
                    }
                }
                // Not a place (e.g. a function name) — plain read, no writeback.
                let r = self.expr(fb, lv)?;
                Ok((r, Writeback::None))
            }
            K::IndexExpr => {
                let parts = exprs(lv);
                let (Some(base), Some(idx)) = (parts.first(), parts.get(1)) else {
                    return Err("index path missing a part".into());
                };
                let (br, parent) = self.read_place(fb, base)?;
                let ir = self.expr(fb, idx)?;
                let t = fb.reg();
                fb.code.push(Instr::Index(t, br, ir));
                Ok((
                    t,
                    Writeback::Index {
                        base: br,
                        idx: ir,
                        parent: Box::new(parent),
                    },
                ))
            }
            K::FieldExpr => {
                let parts = exprs(lv);
                let Some(base) = parts.first() else {
                    return Err("field path missing its base".into());
                };
                let fname = ident_of(lv).unwrap_or_default();
                let (br, parent) = self.read_place(fb, base)?;
                let cidx = fb.const_val(Value::text(fname));
                let t = fb.reg();
                fb.code.push(Instr::GetField(t, br, cidx));
                Ok((
                    t,
                    Writeback::Field {
                        base: br,
                        name: cidx,
                        parent: Box::new(parent),
                    },
                ))
            }
            _ => {
                let r = self.expr(fb, lv)?;
                Ok((r, Writeback::None))
            }
        }
    }

    fn apply_writeback(&mut self, fb: &mut Fb, val: u16, wb: Writeback) {
        match wb {
            Writeback::None => {}
            Writeback::Slot(slot) => {
                if slot != val {
                    fb.code.push(Instr::Move(slot, val));
                }
            }
            Writeback::Global(g) => fb.code.push(Instr::StoreGlobal(g, val)),
            Writeback::Index { base, idx, parent } => {
                fb.code.push(Instr::IndexSet(base, idx, val));
                self.apply_writeback(fb, base, *parent);
            }
            Writeback::Field { base, name, parent } => {
                fb.code.push(Instr::SetField(base, name, val));
                self.apply_writeback(fb, base, *parent);
            }
        }
    }

    // ----- expressions -----

    fn expr(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        match n.kind {
            K::Literal => self.literal(fb, n),
            K::NameRef => self.name_ref(fb, n),
            K::ParenExpr => match exprs(n).first() {
                Some(e) => self.expr(fb, e),
                None => Err("empty parentheses".into()),
            },
            K::BinaryExpr => self.binary(fb, n),
            K::RangeExpr => {
                let parts = exprs(n);
                let a = self.expr(fb, parts.first().ok_or("range missing start")?)?;
                let b = self.expr(fb, parts.get(1).ok_or("range missing end")?)?;
                let dst = fb.reg();
                fb.code.push(Instr::MakeRange(dst, a, b));
                Ok(dst)
            }
            K::UnaryExpr => {
                let inner = self.expr(fb, exprs(n).first().ok_or("unary without operand")?)?;
                let dst = fb.reg();
                if n.find_token(K::NotKw).is_some() {
                    fb.code.push(Instr::Not(dst, inner));
                } else {
                    fb.code.push(Instr::Neg(dst, inner));
                }
                Ok(dst)
            }
            K::CallExpr => self.call(fb, n),
            K::FieldExpr => {
                let parts = exprs(n);
                let base = parts.first().ok_or("field access without a base")?;
                let fname = ident_of(n).ok_or("field access without a name")?;
                if let Some(mid) = self.module_of(fb, base) {
                    let cidx = fb.const_val(Value::text(fname));
                    let dst = fb.reg();
                    fb.code.push(Instr::ModuleConst(dst, mid, cidx));
                    return Ok(dst);
                }
                let br = self.expr(fb, base)?;
                let cidx = fb.const_val(Value::text(fname));
                let dst = fb.reg();
                fb.code.push(Instr::GetField(dst, br, cidx));
                Ok(dst)
            }
            K::IndexExpr => {
                let parts = exprs(n);
                let base = self.expr(fb, parts.first().ok_or("index without a base")?)?;
                let idx = self.expr(fb, parts.get(1).ok_or("index without a position")?)?;
                let dst = fb.reg();
                fb.code.push(Instr::Index(dst, base, idx));
                Ok(dst)
            }
            K::ListExpr => {
                let mut items = Vec::new();
                for e in exprs(n) {
                    items.push(self.expr(fb, e)?);
                }
                let dst = fb.reg();
                fb.code.push(Instr::MakeList(dst, items));
                Ok(dst)
            }
            K::MapExpr => {
                let mut pairs = Vec::new();
                for entry in n.nodes_of(K::MapEntry) {
                    let kv = exprs(entry);
                    let k = self.expr(fb, kv.first().ok_or("map entry without a key")?)?;
                    let v = self.expr(fb, kv.get(1).ok_or("map entry without a value")?)?;
                    pairs.push((k, v));
                }
                let dst = fb.reg();
                fb.code.push(Instr::MakeMap(dst, pairs));
                Ok(dst)
            }
            K::AskExpr => {
                let prompt = self.expr(fb, exprs(n).first().ok_or("ask without a question")?)?;
                let dst = fb.reg();
                fb.code.push(Instr::Ask(dst, prompt));
                Ok(dst)
            }
            K::TryExpr => {
                let parts = exprs(n);
                let inner = self.expr(fb, parts.first().ok_or("try without a value")?)?;
                let dst = fb.reg();
                let fail = fb.emit_patch(Instr::TryUnwrap(dst, inner, 0));
                let end = fb.emit_patch(Instr::Jump(0));
                fb.patch(fail);
                match parts.get(1) {
                    Some(fbk) => {
                        let f = self.expr(fb, fbk)?;
                        fb.code.push(Instr::Move(dst, f));
                    }
                    None => {
                        fb.code.push(Instr::NoneToFail(inner));
                        fb.code.push(Instr::Ret(inner));
                    }
                }
                fb.patch(end);
                Ok(dst)
            }
            _ => Err(format!("cannot compile expression kind {:?}", n.kind)),
        }
    }

    fn literal(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let tok = n
            .child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia())
            .cloned()
            .ok_or("empty literal")?;
        if tok.kind == K::StrLit {
            return self.string_literal(fb, &tok.text);
        }
        let v = literal_value(&tok.kind, &tok.text)?;
        let cidx = fb.const_val(v);
        let dst = fb.reg();
        fb.code.push(Instr::LoadConst(dst, cidx));
        Ok(dst)
    }

    fn string_literal(&mut self, fb: &mut Fb, raw: &str) -> Result<u16, String> {
        let segs = segments(raw);
        let holes = segs.iter().any(|s| matches!(s, Segment::Expr(_)));
        if !holes {
            let text: String = segs
                .into_iter()
                .map(|s| match s {
                    Segment::Text(t) => t,
                    Segment::Expr(_) => String::new(),
                })
                .collect();
            let cidx = fb.const_val(Value::text(text));
            let dst = fb.reg();
            fb.code.push(Instr::LoadConst(dst, cidx));
            return Ok(dst);
        }
        let mut parts = Vec::new();
        for seg in segs {
            match seg {
                Segment::Text(t) => {
                    if t.is_empty() {
                        continue;
                    }
                    let cidx = fb.const_val(Value::text(t));
                    let r = fb.reg();
                    fb.code.push(Instr::LoadConst(r, cidx));
                    parts.push(r);
                }
                Segment::Expr(src) => {
                    let fragment = spider_syntax::parse_expr_source(&src);
                    if !fragment.diagnostics.is_empty() {
                        return Err(format!("invalid interpolation `{{{src}}}`"));
                    }
                    let inner = fragment
                        .root
                        .child_nodes()
                        .into_iter()
                        .find(|c| c.kind.is_expr())
                        .cloned()
                        .ok_or("empty interpolation")?;
                    parts.push(self.expr(fb, &inner)?);
                }
            }
        }
        let dst = fb.reg();
        fb.code.push(Instr::Concat(dst, parts));
        Ok(dst)
    }

    fn name_ref(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let name = ident_of(n).ok_or("nameless reference")?;
        if let Some(r) = fb.lookup(&name) {
            return Ok(r);
        }
        if fb.is_entry {
            if let Some(&g) = self.globals.get(&name) {
                let dst = fb.reg();
                fb.code.push(Instr::LoadGlobal(dst, g));
                return Ok(dst);
            }
        }
        if name == "None" {
            let cidx = fb.const_val(Value::text("None"));
            let dst = fb.reg();
            fb.code.push(Instr::MakeVariant(dst, cidx, Vec::new()));
            return Ok(dst);
        }
        if let Some(v) = self.variants.get(&name) {
            if v.arity == 0 {
                let cidx = fb.const_val(Value::text(name));
                let dst = fb.reg();
                fb.code.push(Instr::MakeVariant(dst, cidx, Vec::new()));
                return Ok(dst);
            }
        }
        if let Some(&idx) = self.fn_ids.get(&self.qualify(&name)) {
            let cidx = fb.const_val(Value::FnRef(idx));
            let dst = fb.reg();
            fb.code.push(Instr::LoadConst(dst, cidx));
            return Ok(dst);
        }
        if self.modules.contains_key(&name) {
            let cidx = fb.const_val(Value::text(format!("<module {name}>")));
            let dst = fb.reg();
            fb.code.push(Instr::LoadConst(dst, cidx));
            return Ok(dst);
        }
        Err(format!("unknown name `{name}` reached the compiler"))
    }

    fn binary(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let parts = exprs(n);
        let (Some(le), Some(re)) = (parts.first(), parts.get(1)) else {
            return Err("binary expression missing a side".into());
        };
        let op_tok = n
            .child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .ok_or("binary expression without an operator")?;
        // Short-circuit and/or.
        if matches!(op_tok, K::AndKw | K::OrKw) {
            let dst = fb.reg();
            let l = self.expr(fb, le)?;
            fb.code.push(Instr::Move(dst, l));
            let skip = if op_tok == K::AndKw {
                fb.emit_patch(Instr::JumpIfFalse(dst, 0))
            } else {
                fb.emit_patch(Instr::JumpIfTrue(dst, 0))
            };
            let r = self.expr(fb, re)?;
            fb.code.push(Instr::Move(dst, r));
            fb.patch(skip);
            return Ok(dst);
        }
        let op = match op_tok {
            K::Plus => BinOp::Add,
            K::Minus => BinOp::Sub,
            K::Star => BinOp::Mul,
            K::Slash => BinOp::Div,
            K::Percent => BinOp::Mod,
            K::EqEq => BinOp::Eq,
            K::NotEq => BinOp::Ne,
            K::Lt => BinOp::Lt,
            K::LtEq => BinOp::Le,
            K::Gt => BinOp::Gt,
            K::GtEq => BinOp::Ge,
            other => return Err(format!("unknown operator {other:?}")),
        };
        let l = self.expr(fb, le)?;
        let r = self.expr(fb, re)?;
        let dst = fb.reg();
        fb.code.push(Instr::Bin(op, dst, l, r));
        Ok(dst)
    }

    fn call(&mut self, fb: &mut Fb, n: &Rc<Node>) -> Result<u16, String> {
        let parts = exprs(n);
        let callee = parts.first().ok_or("call without a callee")?;
        let mut args = Vec::new();
        if let Some(al) = n.find_node(K::ArgList) {
            for a in exprs(al) {
                args.push(self.expr(fb, a)?);
            }
        }

        if callee.kind == K::NameRef {
            let name = ident_of(callee).unwrap_or_default();
            let is_local =
                fb.lookup(&name).is_some() || (fb.is_entry && self.globals.contains_key(&name));
            if !is_local {
                match name.as_str() {
                    "Ok" | "Fail" | "Some" => {
                        let cidx = fb.const_val(Value::text(name));
                        let dst = fb.reg();
                        fb.code.push(Instr::MakeVariant(dst, cidx, args));
                        return Ok(dst);
                    }
                    _ => {}
                }
                if let Some(info) = self.records.get(&name) {
                    let cidx = fb.const_val(Value::Record(Rc::new(crate::value::RecordVal {
                        shape: info.shape.clone(),
                        fields: Vec::new(),
                    })));
                    let dst = fb.reg();
                    fb.code.push(Instr::MakeRecord(dst, cidx, args));
                    return Ok(dst);
                }
                if let Some(v) = self.variants.get(&name) {
                    if v.arity > 0 {
                        let cidx = fb.const_val(Value::text(name));
                        let dst = fb.reg();
                        fb.code.push(Instr::MakeVariant(dst, cidx, args));
                        return Ok(dst);
                    }
                }
                if let Some(&idx) = self.fn_ids.get(&self.qualify(&name)) {
                    let dst = fb.reg();
                    fb.code.push(Instr::Call(dst, idx, args));
                    return Ok(dst);
                }
                if name == "expect" {
                    let cidx = fb.const_val(Value::text("expect"));
                    let dst = fb.reg();
                    fb.code
                        .push(Instr::CallModule(dst, MODULE_BUILTIN, cidx, args));
                    return Ok(dst);
                }
            }
            // A local/global holding a function value.
            let f = self.expr(fb, callee)?;
            let dst = fb.reg();
            fb.code.push(Instr::CallValue(dst, f, args));
            return Ok(dst);
        }

        if callee.kind == K::FieldExpr {
            let cparts = exprs(callee);
            let base = cparts.first().ok_or("method call without a receiver")?;
            let method = ident_of(callee).ok_or("method call without a name")?;
            // Sibling project module: alias.fn(...) -> direct call.
            if base.kind == K::NameRef {
                if let Some(alias) = ident_of(base) {
                    if fb.lookup(&alias).is_none() {
                        if let Some(target) = self.user_import(&alias).cloned() {
                            let key = format!("{target}::{method}");
                            let idx = *self.fn_ids.get(&key).ok_or_else(|| {
                                format!("module `{alias}` has no compiled function `{method}`")
                            })?;
                            let dst = fb.reg();
                            fb.code.push(Instr::Call(dst, idx, args));
                            return Ok(dst);
                        }
                    }
                }
            }
            if let Some(mid) = self.module_of(fb, base) {
                let cidx = fb.const_val(Value::text(method));
                let dst = fb.reg();
                fb.code.push(Instr::CallModule(dst, mid, cidx, args));
                return Ok(dst);
            }
            let mutating = MUTATING_METHODS.contains(&method.as_str());
            let (recv, wb) = if mutating {
                self.read_place(fb, base)?
            } else {
                (self.expr(fb, base)?, Writeback::None)
            };
            let cidx = fb.const_val(Value::text(method));
            let dst = fb.reg();
            fb.code.push(Instr::CallMethod(dst, recv, cidx, args));
            if mutating {
                self.apply_writeback(fb, recv, wb);
            }
            return Ok(dst);
        }

        let f = self.expr(fb, callee)?;
        let dst = fb.reg();
        fb.code.push(Instr::CallValue(dst, f, args));
        Ok(dst)
    }

    /// If `base` names an imported module (not shadowed), its module id.
    fn module_of(&self, fb: &Fb, base: &Rc<Node>) -> Option<u8> {
        if base.kind != K::NameRef {
            return None;
        }
        let name = ident_of(base)?;
        if fb.lookup(&name).is_some() {
            return None;
        }
        if fb.is_entry && self.globals.contains_key(&name) {
            return None;
        }
        self.modules.get(&name).copied()
    }

    fn global_slot(&mut self, name: &str) -> u16 {
        if let Some(&g) = self.globals.get(name) {
            return g;
        }
        let g = self
            .globals
            .values()
            .copied()
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        self.globals.insert(name.to_string(), g);
        g
    }
}

enum Writeback {
    None,
    Slot(u16),
    Global(u16),
    Index {
        base: u16,
        idx: u16,
        parent: Box<Writeback>,
    },
    Field {
        base: u16,
        name: u32,
        parent: Box<Writeback>,
    },
}

struct Fb {
    code: Vec<Instr>,
    consts: Vec<Value>,
    next_reg: u16,
    max_reg: u16,
    scopes: Vec<HashMap<String, u16>>,
    is_entry: bool,
}

impl Fb {
    fn new(n_params: u16, is_entry: bool) -> Fb {
        Fb {
            code: Vec::new(),
            consts: Vec::new(),
            next_reg: n_params,
            max_reg: n_params,
            scopes: vec![HashMap::new()],
            is_entry,
        }
    }

    fn reg(&mut self) -> u16 {
        let r = self.next_reg;
        self.next_reg += 1;
        self.max_reg = self.max_reg.max(self.next_reg);
        r
    }

    fn bind(&mut self, name: &str, reg: u16) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), reg);
        }
    }

    fn lookup(&self, name: &str) -> Option<u16> {
        for s in self.scopes.iter().rev() {
            if let Some(&r) = s.get(name) {
                return Some(r);
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn const_val(&mut self, v: Value) -> u32 {
        // Dedup the common repeats (tags, small ints).
        for (i, existing) in self.consts.iter().enumerate() {
            if crate::value::value_eq(existing, &v) {
                if let (Value::Record(a), Value::Record(b)) = (existing, &v) {
                    if !Rc::ptr_eq(&a.shape, &b.shape) {
                        continue;
                    }
                }
                return i as u32;
            }
        }
        self.consts.push(v);
        (self.consts.len() - 1) as u32
    }

    fn emit_patch(&mut self, instr: Instr) -> usize {
        self.code.push(instr);
        self.code.len() - 1
    }

    /// Points the jump at `at` to the current end of code.
    fn patch(&mut self, at: usize) {
        let target = self.code.len() as u32;
        match &mut self.code[at] {
            Instr::Jump(t)
            | Instr::JumpIfFalse(_, t)
            | Instr::JumpIfTrue(_, t)
            | Instr::TryUnwrap(_, _, t)
            | Instr::IterNext(_, _, t) => *t = target,
            _ => {}
        }
    }

    fn store(self, proto: &mut FnProto) {
        proto.code = self.code;
        proto.consts = self.consts;
        proto.n_regs = self.max_reg.max(proto.n_params);
    }
}

fn ident_of(n: &Node) -> Option<String> {
    n.find_token(K::Ident).map(|t| t.text.clone())
}

fn exprs(n: &Node) -> Vec<&Rc<Node>> {
    n.child_nodes()
        .into_iter()
        .filter(|c| c.kind.is_expr())
        .collect()
}

fn literal_value(kind: &K, text: &str) -> Result<Value, String> {
    match kind {
        K::IntLit => text
            .replace('_', "")
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("the number {text} is too large for Int")),
        K::FloatLit => text
            .replace('_', "")
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("cannot read the number {text}")),
        K::TrueKw => Ok(Value::Bool(true)),
        K::FalseKw => Ok(Value::Bool(false)),
        other => Err(format!("not a literal: {other:?}")),
    }
}
