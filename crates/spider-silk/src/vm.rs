//! The Silk virtual machine.
//!
//! Executes checker-approved bytecode. Expected failures in user programs
//! become `RuntimeError`s with authored E03xx codes (never Rust panics);
//! states the checker makes unreachable surface as `internal` errors so a
//! toolchain bug can never masquerade as a user mistake.

use crate::compile::{
    BinOp, Instr, Program, MODULE_BUILTIN, MODULE_FILES, MODULE_MATH, MODULE_RANDOM,
};
use crate::value::{display, value_cmp, value_eq, Iter, RecordVal, Value, VariantVal};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub code: &'static str,
    pub message: String,
}

fn err(code: &'static str, message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        code,
        message: message.into(),
    }
}

fn internal(message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        code: "internal",
        message: message.into(),
    }
}

pub trait Io {
    fn say(&mut self, line: &str);
    fn ask(&mut self, prompt: &str) -> String;
}

pub struct ConsoleIo;

impl Io for ConsoleIo {
    fn say(&mut self, line: &str) {
        println!("{line}");
    }
    fn ask(&mut self, prompt: &str) -> String {
        use std::io::Write;
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        line.trim_end_matches(['\r', '\n']).to_string()
    }
}

#[derive(Default)]
pub struct CaptureIo {
    pub out: String,
    pub inputs: VecDeque<String>,
}

impl Io for CaptureIo {
    fn say(&mut self, line: &str) {
        self.out.push_str(line);
        self.out.push('\n');
    }
    fn ask(&mut self, _prompt: &str) -> String {
        self.inputs.pop_front().unwrap_or_default()
    }
}

// Python-precedent teaching default. The VM currently recurses on the host
// stack; an explicit frame stack (and a higher limit) is M8 performance work.
const MAX_CALL_DEPTH: u32 = 1000;

/// One Spider call frame, heap-allocated on the VM's own stack — Spider
/// recursion can never overflow the host stack.
struct Frame {
    fidx: u32,
    pc: usize,
    regs: Vec<Value>,
    /// Register in the *caller's* frame that receives this frame's value.
    ret_dst: u16,
}

impl Frame {
    fn new(p: &Program, fidx: u32, args: Vec<Value>) -> Result<Frame, RuntimeError> {
        let proto = p
            .protos
            .get(fidx as usize)
            .ok_or_else(|| internal("missing function"))?;
        let mut regs = vec![Value::Unit; proto.n_regs.max(proto.n_params) as usize + 1];
        for (i, a) in args.into_iter().enumerate() {
            if i < regs.len() {
                regs[i] = a;
            }
        }
        Ok(Frame {
            fidx,
            pc: 0,
            regs,
            ret_dst: 0,
        })
    }
}

pub struct Vm<'a> {
    pub globals: Vec<Value>,
    /// Runtime capability grants — the second enforcement layer (SRS FR-21).
    /// The checker's E0244 is advisory for humans; this one is law.
    pub allowed: HashSet<String>,
    io: &'a mut dyn Io,
    rng: u64,
    depth: u32,
}

impl<'a> Vm<'a> {
    pub fn new(io: &'a mut dyn Io) -> Vm<'a> {
        Vm {
            globals: Vec::new(),
            allowed: HashSet::new(),
            io,
            rng: 0x9E37_79B9_7F4A_7C15,
            depth: 0,
        }
    }

    /// Runs one `test "…"` proto (after `run_entry` has done setup).
    pub fn call_proto(&mut self, p: &Program, idx: u32) -> Result<Value, RuntimeError> {
        self.call(p, idx, Vec::new())
    }

    fn require_cap(&self, cap: &str, what: &str) -> Result<(), RuntimeError> {
        if self.allowed.contains(cap) {
            Ok(())
        } else {
            Err(err(
                "E0310",
                format!("{what} needs the `{cap}` capability, which this program was not given"),
            ))
        }
    }

    /// Runs the script, then auto-calls `fn main()` if one exists (ADR-013).
    pub fn run(&mut self, p: &Program) -> Result<Value, RuntimeError> {
        let v = self.run_entry(p)?;
        if let Some(main) = p.main {
            self.call(p, main, Vec::new())?;
        }
        Ok(v)
    }

    /// Runs only the script statements — the REPL path, where a defined
    /// `main` must not re-fire on every entry.
    pub fn run_entry(&mut self, p: &Program) -> Result<Value, RuntimeError> {
        if self.globals.len() < p.n_globals as usize {
            self.globals.resize(p.n_globals as usize, Value::Unit);
        }
        self.call(p, p.entry, Vec::new())
    }

    /// The interpreter loop. Call frames live on an explicit heap stack, so
    /// Spider recursion never consumes host stack: the recursion limit is a
    /// language rule (E0307), not a crash.
    fn call(&mut self, p: &Program, fidx: u32, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let mut stack: Vec<Frame> = Vec::new();
        stack.push(Frame::new(p, fidx, args)?);

        loop {
            let fi = stack.len() - 1;
            let proto = &p.protos[stack[fi].fidx as usize];
            let consts = &proto.consts;
            let pc = stack[fi].pc;

            if pc >= proto.code.len() {
                // Fell off the end: implicit `return nothing`.
                match self.deliver(&mut stack, Value::Unit) {
                    Some(v) => return Ok(v),
                    None => continue,
                }
            }
            stack[fi].pc = pc + 1;

            macro_rules! reg {
                ($i:expr) => {
                    stack[fi].regs[$i as usize].clone()
                };
            }
            macro_rules! set {
                ($i:expr, $v:expr) => {{
                    let v = $v;
                    stack[fi].regs[$i as usize] = v;
                }};
            }

            match &proto.code[pc] {
                Instr::LoadConst(dst, c) => set!(*dst, consts[*c as usize].clone()),
                Instr::LoadUnit(dst) => set!(*dst, Value::Unit),
                Instr::Move(dst, src) => set!(*dst, reg!(*src)),
                Instr::LoadGlobal(dst, g) => {
                    set!(
                        *dst,
                        self.globals.get(*g as usize).cloned().unwrap_or(Value::Unit)
                    )
                }
                Instr::StoreGlobal(g, src) => {
                    let gi = *g as usize;
                    if self.globals.len() <= gi {
                        self.globals.resize(gi + 1, Value::Unit);
                    }
                    self.globals[gi] = reg!(*src);
                }
                Instr::Bin(op, dst, a, b) => {
                    let va = reg!(*a);
                    let vb = reg!(*b);
                    set!(*dst, bin(*op, &va, &vb)?);
                }
                Instr::Neg(dst, a) => {
                    let v = match reg!(*a) {
                        Value::Int(i) => Value::Int(i.checked_neg().ok_or_else(|| {
                            err("E0302", "negating this number overflows Int")
                        })?),
                        Value::Float(f) => Value::Float(-f),
                        other => return Err(internal(format!("negating {}", other.kind_name()))),
                    };
                    set!(*dst, v);
                }
                Instr::Not(dst, a) => {
                    let v = match reg!(*a) {
                        Value::Bool(b) => Value::Bool(!b),
                        other => return Err(internal(format!("not on {}", other.kind_name()))),
                    };
                    set!(*dst, v);
                }
                Instr::Jump(t) => stack[fi].pc = *t as usize,
                Instr::JumpIfFalse(c, t) => {
                    if matches!(reg!(*c), Value::Bool(false)) {
                        stack[fi].pc = *t as usize;
                    }
                }
                Instr::JumpIfTrue(c, t) => {
                    if matches!(reg!(*c), Value::Bool(true)) {
                        stack[fi].pc = *t as usize;
                    }
                }
                Instr::Call(dst, f, arg_regs) => {
                    let args: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    self.push_frame(p, &mut stack, *f, args, *dst)?;
                }
                Instr::CallValue(dst, callee, arg_regs) => {
                    let args: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    match reg!(*callee) {
                        Value::FnRef(f) => self.push_frame(p, &mut stack, f, args, *dst)?,
                        other => {
                            return Err(internal(format!(
                                "calling {} as a function",
                                other.kind_name()
                            )))
                        }
                    }
                }
                Instr::CallMethod(dst, recv, name_c, arg_regs) => {
                    let name = const_text(consts, *name_c)?;
                    let args: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    let out = method(&mut stack[fi].regs[*recv as usize], &name, args)?;
                    set!(*dst, out);
                }
                Instr::CallModule(dst, module, name_c, arg_regs) => {
                    let name = const_text(consts, *name_c)?;
                    let args: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    set!(*dst, self.module_call(*module, &name, args)?);
                }
                Instr::ModuleConst(dst, module, name_c) => {
                    let name = const_text(consts, *name_c)?;
                    set!(*dst, module_const(*module, &name)?);
                }
                Instr::MakeList(dst, items) => {
                    let vals: Vec<Value> = items.iter().map(|i| reg!(*i)).collect();
                    set!(*dst, Value::List(Rc::new(vals)));
                }
                Instr::MakeMap(dst, pairs) => {
                    let mut vals = Vec::new();
                    for (k, v) in pairs {
                        vals.push((reg!(*k), reg!(*v)));
                    }
                    set!(*dst, Value::Map(Rc::new(vals)));
                }
                Instr::MakeRange(dst, a, b) => match (reg!(*a), reg!(*b)) {
                    (Value::Int(x), Value::Int(y)) => set!(*dst, Value::Range(x, y)),
                    _ => return Err(internal("range ends must be Int")),
                },
                Instr::Index(dst, base, idx) => {
                    let b = reg!(*base);
                    let i = reg!(*idx);
                    set!(*dst, index_get(&b, &i)?);
                }
                Instr::IndexSet(base, idx, val) => {
                    let i = reg!(*idx);
                    let v = reg!(*val);
                    index_set(&mut stack[fi].regs[*base as usize], i, v)?;
                }
                Instr::GetField(dst, obj, name_c) => {
                    let name = const_text(consts, *name_c)?;
                    let o = reg!(*obj);
                    set!(*dst, field_get(&o, &name)?);
                }
                Instr::SetField(obj, name_c, val) => {
                    let name = const_text(consts, *name_c)?;
                    let v = reg!(*val);
                    field_set(&mut stack[fi].regs[*obj as usize], &name, v)?;
                }
                Instr::MakeRecord(dst, shape_c, arg_regs) => {
                    let shape = match &consts[*shape_c as usize] {
                        Value::Record(r) => r.shape.clone(),
                        _ => return Err(internal("record shape const missing")),
                    };
                    let fields: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    set!(*dst, Value::Record(Rc::new(RecordVal { shape, fields })));
                }
                Instr::MakeVariant(dst, tag_c, arg_regs) => {
                    let tag = const_text(consts, *tag_c)?;
                    let fields: Vec<Value> = arg_regs.iter().map(|a| reg!(*a)).collect();
                    set!(
                        *dst,
                        Value::Variant(Rc::new(VariantVal {
                            tag: Rc::from(tag.as_str()),
                            fields,
                        }))
                    );
                }
                Instr::TestTag(dst, val, tag_c) => {
                    let tag = const_text(consts, *tag_c)?;
                    let matched = match reg!(*val) {
                        Value::Variant(v) => *v.tag == *tag,
                        _ => false,
                    };
                    set!(*dst, Value::Bool(matched));
                }
                Instr::GetVariantField(dst, val, i) => {
                    let v = match reg!(*val) {
                        Value::Variant(v) => {
                            v.fields.get(*i as usize).cloned().unwrap_or(Value::Unit)
                        }
                        _ => return Err(internal("unpacking a non-choice value")),
                    };
                    set!(*dst, v);
                }
                Instr::TryUnwrap(dst, src, fail) => match reg!(*src) {
                    Value::Variant(v) if matches!(&*v.tag, "Ok" | "Some") => {
                        set!(*dst, v.fields.first().cloned().unwrap_or(Value::Unit));
                    }
                    Value::Variant(v) if matches!(&*v.tag, "Fail" | "None") => {
                        stack[fi].pc = *fail as usize;
                    }
                    other => {
                        return Err(internal(format!(
                            "try on {} reached the runtime",
                            other.kind_name()
                        )))
                    }
                },
                Instr::NoneToFail(r) => {
                    if let Value::Variant(v) = &stack[fi].regs[*r as usize] {
                        if &*v.tag == "None" {
                            stack[fi].regs[*r as usize] =
                                Value::variant("Fail", vec![Value::text("the value was missing")]);
                        }
                    }
                }
                Instr::IterNew(dst, src) => {
                    let it = match reg!(*src) {
                        Value::List(items) => Iter::List { items, idx: 0 },
                        Value::Range(a, b) => Iter::Range { cur: a, end: b },
                        other => {
                            return Err(internal(format!("looping over {}", other.kind_name())))
                        }
                    };
                    set!(*dst, Value::Iter(Rc::new(RefCell::new(it))));
                }
                Instr::IterNext(item, iter, end) => {
                    let next = match &stack[fi].regs[*iter as usize] {
                        Value::Iter(state) => {
                            let mut s = state.borrow_mut();
                            match &mut *s {
                                Iter::List { items, idx } => {
                                    if *idx < items.len() {
                                        let v = items[*idx].clone();
                                        *idx += 1;
                                        Some(v)
                                    } else {
                                        None
                                    }
                                }
                                Iter::Range { cur, end } => {
                                    if *cur <= *end {
                                        let v = Value::Int(*cur);
                                        *cur += 1;
                                        Some(v)
                                    } else {
                                        None
                                    }
                                }
                            }
                        }
                        _ => return Err(internal("iterating a non-iterator")),
                    };
                    match next {
                        Some(v) => set!(*item, v),
                        None => stack[fi].pc = *end as usize,
                    }
                }
                Instr::Concat(dst, parts) => {
                    let mut s = String::new();
                    for part in parts {
                        s.push_str(&display(&reg!(*part), false));
                    }
                    set!(*dst, Value::text(s));
                }
                Instr::Say(src) => {
                    let line = display(&reg!(*src), false);
                    self.io.say(&line);
                }
                Instr::Ask(dst, prompt) => {
                    let q = display(&reg!(*prompt), false);
                    let answer = self.io.ask(&q);
                    set!(*dst, Value::text(answer));
                }
                Instr::Ret(src) => {
                    let v = reg!(*src);
                    match self.deliver(&mut stack, v) {
                        Some(v) => return Ok(v),
                        None => continue,
                    }
                }
                Instr::RetUnit => match self.deliver(&mut stack, Value::Unit) {
                    Some(v) => return Ok(v),
                    None => continue,
                },
            }
        }
    }

    fn push_frame(
        &mut self,
        p: &Program,
        stack: &mut Vec<Frame>,
        fidx: u32,
        args: Vec<Value>,
        ret_dst: u16,
    ) -> Result<(), RuntimeError> {
        if stack.len() as u32 >= MAX_CALL_DEPTH + self.depth {
            return Err(err(
                "E0307",
                "the program called functions too deeply (1000 nested calls)",
            ));
        }
        let mut frame = Frame::new(p, fidx, args)?;
        frame.ret_dst = ret_dst;
        stack.push(frame);
        Ok(())
    }

    /// Pops the finished frame and writes its value into the caller.
    /// Returns `Some(value)` when the outermost frame finished.
    fn deliver(&mut self, stack: &mut Vec<Frame>, v: Value) -> Option<Value> {
        let finished = stack.pop().expect("deliver on empty stack");
        match stack.last_mut() {
            Some(parent) => {
                let dst = finished.ret_dst as usize;
                if dst < parent.regs.len() {
                    parent.regs[dst] = v;
                }
                None
            }
            None => Some(v),
        }
    }

    fn module_call(
        &mut self,
        module: u8,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let arg_f = |i: usize| -> Result<f64, RuntimeError> {
            match args.get(i) {
                Some(Value::Float(f)) => Ok(*f),
                Some(Value::Int(n)) => Ok(*n as f64),
                _ => Err(err("E0306", format!("`{name}` needs number arguments"))),
            }
        };
        match (module, name) {
            (MODULE_MATH, "sqrt") => Ok(Value::Float(arg_f(0)?.sqrt())),
            (MODULE_MATH, "pow") => Ok(Value::Float(arg_f(0)?.powf(arg_f(1)?))),
            (MODULE_MATH, "abs") => match args.first() {
                Some(Value::Int(i)) => Ok(Value::Int(i.wrapping_abs())),
                _ => Ok(Value::Float(arg_f(0)?.abs())),
            },
            (MODULE_MATH, "floor") => Ok(Value::Int(arg_f(0)?.floor() as i64)),
            (MODULE_MATH, "round") => Ok(Value::Int(arg_f(0)?.round() as i64)),
            (MODULE_MATH, "min") | (MODULE_MATH, "max") => {
                let (a, b) = (args.first(), args.get(1));
                let (Some(a), Some(b)) = (a, b) else {
                    return Err(err("E0306", format!("`{name}` needs two arguments")));
                };
                match value_cmp(a, b) {
                    Some(ord) => {
                        let take_a = if name == "min" {
                            ord != Ordering::Greater
                        } else {
                            ord != Ordering::Less
                        };
                        Ok(if take_a { a.clone() } else { b.clone() })
                    }
                    None => Err(err("E0305", "these values have no smaller-or-larger order")),
                }
            }
            (MODULE_RANDOM, "seed") => {
                if let Some(Value::Int(s)) = args.first() {
                    self.rng = (*s as u64) | 1;
                }
                Ok(Value::Unit)
            }
            (MODULE_RANDOM, "int") => {
                let (a, b) = match (args.first(), args.get(1)) {
                    (Some(Value::Int(a)), Some(Value::Int(b))) => (*a, *b),
                    _ => return Err(err("E0306", "`random.int` needs two Int bounds")),
                };
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let span = (hi - lo).unsigned_abs().saturating_add(1);
                Ok(Value::Int(lo + (self.next_rng() % span) as i64))
            }
            (MODULE_RANDOM, "float") => {
                Ok(Value::Float((self.next_rng() >> 11) as f64 / (1u64 << 53) as f64))
            }
            (MODULE_FILES, _) => self.files_call(name, args),
            (MODULE_BUILTIN, "expect") => {
                let (Some(actual), Some(expected)) = (args.first(), args.get(1)) else {
                    return Err(internal("expect needs two values"));
                };
                if value_eq(actual, expected) {
                    Ok(Value::Unit)
                } else {
                    Err(err(
                        "E0311",
                        format!(
                            "expected {}, but got {}",
                            display(expected, true),
                            display(actual, true)
                        ),
                    ))
                }
            }
            _ => Err(err(
                "E0306",
                format!("this module has no member named `{name}` in this Spider version"),
            )),
        }
    }

    fn files_call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.require_cap("fs", &format!("`files.{name}`"))?;
        let path_arg = |i: usize| -> Result<String, RuntimeError> {
            match args.get(i) {
                Some(Value::Text(s)) => Ok(s.as_str().to_string()),
                _ => Err(internal("files paths must be Text")),
            }
        };
        let fail = |msg: String| Value::variant("Fail", vec![Value::text(msg)]);
        match name {
            "read_text" => {
                let path = path_arg(0)?;
                Ok(match std::fs::read_to_string(&path) {
                    Ok(text) => Value::variant("Ok", vec![Value::text(text)]),
                    Err(e) => fail(format!("cannot read {path}: {e}")),
                })
            }
            "write_text" => {
                let path = path_arg(0)?;
                let content = match args.get(1) {
                    Some(Value::Text(s)) => s.as_str().to_string(),
                    Some(other) => display(other, false),
                    None => String::new(),
                };
                Ok(match std::fs::write(&path, content) {
                    Ok(()) => Value::variant("Ok", vec![Value::Bool(true)]),
                    Err(e) => fail(format!("cannot write {path}: {e}")),
                })
            }
            "exists" => {
                let path = path_arg(0)?;
                Ok(Value::Bool(std::path::Path::new(&path).exists()))
            }
            "list" => {
                let path = path_arg(0)?;
                Ok(match std::fs::read_dir(&path) {
                    Ok(entries) => {
                        let mut names: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .filter_map(|e| e.file_name().into_string().ok())
                            .collect();
                        names.sort();
                        Value::variant(
                            "Ok",
                            vec![Value::List(Rc::new(
                                names.into_iter().map(Value::text).collect(),
                            ))],
                        )
                    }
                    Err(e) => fail(format!("cannot list {path}: {e}")),
                })
            }
            other => Err(err(
                "E0306",
                format!("`files` has no member named `{other}`"),
            )),
        }
    }

    fn next_rng(&mut self) -> u64 {
        // xorshift64* — deterministic and seedable (classroom requirement).
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn const_text(consts: &[Value], idx: u32) -> Result<String, RuntimeError> {
    match consts.get(idx as usize) {
        Some(Value::Text(s)) => Ok(s.as_str().to_string()),
        _ => Err(internal("missing text constant")),
    }
}

fn module_const(module: u8, name: &str) -> Result<Value, RuntimeError> {
    match (module, name) {
        (MODULE_MATH, "pi") => Ok(Value::Float(std::f64::consts::PI)),
        _ => Err(err(
            "E0306",
            format!("this module has no member named `{name}` (M3 runs math and random)"),
        )),
    }
}

fn bin(op: BinOp, a: &Value, b: &Value) -> Result<Value, RuntimeError> {
    use BinOp::*;
    match op {
        Eq => return Ok(Value::Bool(value_eq(a, b))),
        Ne => return Ok(Value::Bool(!value_eq(a, b))),
        Lt | Le | Gt | Ge => {
            let ord = value_cmp(a, b)
                .ok_or_else(|| internal("comparing unordered values"))?;
            let res = match op {
                Lt => ord == Ordering::Less,
                Le => ord != Ordering::Greater,
                Gt => ord == Ordering::Greater,
                Ge => ord != Ordering::Less,
                _ => unreachable!(),
            };
            return Ok(Value::Bool(res));
        }
        _ => {}
    }
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            let (x, y) = (*x, *y);
            let out = match op {
                Add => x.checked_add(y),
                Sub => x.checked_sub(y),
                Mul => x.checked_mul(y),
                Div => {
                    if y == 0 {
                        return Err(err("E0301", format!("{x} cannot be divided by zero")));
                    }
                    x.checked_div(y)
                }
                Mod => {
                    if y == 0 {
                        return Err(err("E0301", format!("{x} cannot be divided by zero")));
                    }
                    x.checked_rem(y)
                }
                _ => unreachable!(),
            };
            out.map(Value::Int).ok_or_else(|| {
                err(
                    "E0302",
                    format!("the whole-number calculation {x} … {y} grew past what Int can hold"),
                )
            })
        }
        (Value::Float(x), Value::Float(y)) => {
            let out = match op {
                Add => x + y,
                Sub => x - y,
                Mul => x * y,
                Div => x / y,
                Mod => x % y,
                _ => unreachable!(),
            };
            Ok(Value::Float(out))
        }
        _ => Err(internal(format!(
            "arithmetic on {} and {}",
            a.kind_name(),
            b.kind_name()
        ))),
    }
}

fn index_get(base: &Value, idx: &Value) -> Result<Value, RuntimeError> {
    match (base, idx) {
        (Value::List(items), Value::Int(i)) => {
            if *i < 0 || *i as usize >= items.len() {
                return Err(err(
                    "E0303",
                    format!(
                        "the list has {} item(s) (positions 0 to {}), but position {i} was asked for",
                        items.len(),
                        items.len() as i64 - 1
                    ),
                ));
            }
            Ok(items[*i as usize].clone())
        }
        (Value::Map(pairs), key) => pairs
            .iter()
            .find(|(k, _)| value_eq(k, key))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                err(
                    "E0304",
                    format!("the map has no key {}", display(key, true)),
                )
            }),
        _ => Err(internal(format!("indexing {}", base.kind_name()))),
    }
}

fn index_set(base: &mut Value, idx: Value, val: Value) -> Result<(), RuntimeError> {
    match base {
        Value::List(items) => {
            let i = match idx {
                Value::Int(i) => i,
                _ => return Err(internal("list position must be Int")),
            };
            let items_mut = Rc::make_mut(items);
            if i < 0 || i as usize >= items_mut.len() {
                return Err(err(
                    "E0303",
                    format!(
                        "the list has {} item(s), so position {i} cannot be set",
                        items_mut.len()
                    ),
                ));
            }
            items_mut[i as usize] = val;
            Ok(())
        }
        Value::Map(pairs) => {
            let pairs_mut = Rc::make_mut(pairs);
            match pairs_mut.iter_mut().find(|(k, _)| value_eq(k, &idx)) {
                Some((_, v)) => *v = val,
                None => pairs_mut.push((idx, val)),
            }
            Ok(())
        }
        other => Err(internal(format!("index-assign on {}", other.kind_name()))),
    }
}

fn field_get(obj: &Value, name: &str) -> Result<Value, RuntimeError> {
    match obj {
        Value::Record(r) => {
            let pos = r
                .shape
                .fields
                .iter()
                .position(|f| f == name)
                .ok_or_else(|| internal(format!("no field {name}")))?;
            Ok(r.fields.get(pos).cloned().unwrap_or(Value::Unit))
        }
        other => Err(internal(format!("field on {}", other.kind_name()))),
    }
}

fn field_set(obj: &mut Value, name: &str, val: Value) -> Result<(), RuntimeError> {
    match obj {
        Value::Record(r) => {
            let rec = Rc::make_mut(r);
            let pos = rec
                .shape
                .fields
                .iter()
                .position(|f| f == name)
                .ok_or_else(|| internal(format!("no field {name}")))?;
            if pos < rec.fields.len() {
                rec.fields[pos] = val;
            }
            Ok(())
        }
        other => Err(internal(format!("field-assign on {}", other.kind_name()))),
    }
}

/// Built-in methods. Mutating methods operate on the receiver register in
/// place (copy-on-write), and the compiler writes it back through its path.
fn method(recv: &mut Value, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (&mut *recv, name) {
        (Value::Int(i), "to_float") => Ok(Value::Float(*i as f64)),
        (Value::Int(i), "abs") => Ok(Value::Int(i.wrapping_abs())),
        (Value::Float(f), "to_int") => Ok(Value::Int(*f as i64)),
        (Value::Float(f), "round") => Ok(Value::Int(f.round() as i64)),
        (Value::Float(f), "floor") => Ok(Value::Int(f.floor() as i64)),
        (Value::Float(f), "abs") => Ok(Value::Float(f.abs())),
        (Value::Text(s), "length") => Ok(Value::Int(s.chars().count() as i64)),
        (Value::Text(s), "upper") => Ok(Value::text(s.to_uppercase())),
        (Value::Text(s), "lower") => Ok(Value::text(s.to_lowercase())),
        (Value::Text(s), "trim") => Ok(Value::text(s.trim())),
        (Value::Text(s), "contains") => match args.first() {
            Some(Value::Text(needle)) => Ok(Value::Bool(s.contains(needle.as_str()))),
            _ => Err(internal("contains needs Text")),
        },
        (Value::Text(s), "split") => match args.first() {
            Some(Value::Text(sep)) => {
                let parts: Vec<Value> = if sep.is_empty() {
                    s.chars().map(|c| Value::text(c.to_string())).collect()
                } else {
                    s.split(sep.as_str()).map(Value::text).collect()
                };
                Ok(Value::List(Rc::new(parts)))
            }
            _ => Err(internal("split needs Text")),
        },
        (Value::List(items), "length") => Ok(Value::Int(items.len() as i64)),
        (Value::List(items), "first") => Ok(match items.first() {
            Some(v) => Value::variant("Some", vec![v.clone()]),
            None => Value::variant("None", vec![]),
        }),
        (Value::List(items), "last") => Ok(match items.last() {
            Some(v) => Value::variant("Some", vec![v.clone()]),
            None => Value::variant("None", vec![]),
        }),
        (Value::List(items), "contains") => {
            let needle = args.first().cloned().unwrap_or(Value::Unit);
            Ok(Value::Bool(items.iter().any(|v| value_eq(v, &needle))))
        }
        (Value::List(items), "reverse") => {
            let mut v: Vec<Value> = items.as_ref().clone();
            v.reverse();
            Ok(Value::List(Rc::new(v)))
        }
        (Value::List(items), "sort") => {
            let v: Vec<Value> = items.as_ref().clone();
            for pair in v.windows(2) {
                if value_cmp(&pair[0], &pair[1]).is_none() {
                    return Err(err(
                        "E0305",
                        "the items in this list have no smaller-or-larger order, so it cannot be sorted",
                    ));
                }
            }
            let mut sorted = v;
            sorted.sort_by(|a, b| value_cmp(a, b).unwrap_or(Ordering::Equal));
            Ok(Value::List(Rc::new(sorted)))
        }
        (Value::List(items), "push") => {
            let v = args.into_iter().next().unwrap_or(Value::Unit);
            Rc::make_mut(items).push(v);
            Ok(Value::Unit)
        }
        (Value::Map(pairs), "length") => Ok(Value::Int(pairs.len() as i64)),
        (Value::Map(pairs), "keys") => Ok(Value::List(Rc::new(
            pairs.iter().map(|(k, _)| k.clone()).collect(),
        ))),
        (Value::Map(pairs), "values") => Ok(Value::List(Rc::new(
            pairs.iter().map(|(_, v)| v.clone()).collect(),
        ))),
        (Value::Map(pairs), "has") => {
            let key = args.first().cloned().unwrap_or(Value::Unit);
            Ok(Value::Bool(pairs.iter().any(|(k, _)| value_eq(k, &key))))
        }
        (recv, other) => Err(internal(format!(
            "{} has no method `{other}` at runtime",
            recv.kind_name()
        ))),
    }
}

pub fn render_panic(e: &RuntimeError) -> String {
    let mut out = String::new();
    if e.code == "internal" {
        out.push_str("internal Spider error (this is a bug in Spider, not in your code):\n");
        out.push_str(&format!("  {}\n", e.message));
        out.push_str("please report it: https://github.com/spider-lang/spider/issues\n");
        return out;
    }
    out.push_str(&format!("panic[{}]: {}\n", e.code, e.message));
    if let Some(x) = spider_syntax::explain(e.code) {
        out.push_str(&format!("what happened: {}\n", x.what));
        out.push_str(&format!("why: {}\n", x.why));
        out.push_str(&format!("how to fix: {}\n", x.fix));
    }
    out.push_str(&format!("learn more: spider explain {}\n", e.code));
    out
}
