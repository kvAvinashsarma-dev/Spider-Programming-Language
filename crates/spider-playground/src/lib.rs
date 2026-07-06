//! The browser playground engine: the *actual* Spider toolchain —
//! parser, checker, and Silk VM — compiled to wasm32-unknown-unknown.
//! No emulation, no JavaScript reimplementation: what runs on the website
//! is what runs in `spider run`.
//!
//! ABI (no wasm-bindgen; the zero-dependency rule holds in the browser too):
//!   sp_alloc(len)          -> ptr   caller writes UTF-8 into it
//!   sp_run(src_ptr, src_len, input_ptr, input_len) -> out_len
//!   sp_out_ptr()           -> ptr   UTF-8 result, `out_len` bytes
//!
//! Result text starts with one status line: "OK", "ERR" (diagnostics), or
//! "PANIC" (runtime), followed by the program output / rendered messages.

use std::cell::RefCell;

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static BUFFERS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn sp_alloc(len: i32) -> *mut u8 {
    let mut buf = vec![0u8; len.max(0) as usize];
    let ptr = buf.as_mut_ptr();
    BUFFERS.with(|b| b.borrow_mut().push(buf));
    ptr
}

#[no_mangle]
pub extern "C" fn sp_out_ptr() -> *const u8 {
    OUT.with(|o| o.borrow().as_ptr())
}

/// # Safety
/// `src_ptr`/`input_ptr` must come from `sp_alloc` with the given lengths.
#[no_mangle]
pub unsafe extern "C" fn sp_run(
    src_ptr: *const u8,
    src_len: i32,
    input_ptr: *const u8,
    input_len: i32,
) -> i32 {
    let src = read_str(src_ptr, src_len);
    let inputs = read_str(input_ptr, input_len);
    let result = run_playground(&src, &inputs);
    let len = result.len() as i32;
    OUT.with(|o| *o.borrow_mut() = result.into_bytes());
    BUFFERS.with(|b| b.borrow_mut().clear());
    len
}

unsafe fn read_str(ptr: *const u8, len: i32) -> String {
    if ptr.is_null() || len <= 0 {
        return String::new();
    }
    let slice = std::slice::from_raw_parts(ptr, len as usize);
    String::from_utf8_lossy(slice).into_owned()
}

pub fn run_playground(src: &str, inputs: &str) -> String {
    let file = "playground.sp";
    match spider_silk::prepare(src) {
        Ok(prepared) => {
            let mut io = spider_silk::CaptureIo::default();
            for line in inputs.lines() {
                io.inputs.push_back(line.to_string());
            }
            let mut vm = spider_silk::Vm::new(&mut io);
            // Safe Mode in the browser too: zero capabilities.
            match vm.run(&prepared.program) {
                Ok(_) => {
                    let mut out = String::from("OK\n");
                    for w in &prepared.warnings {
                        out.push_str(&spider_syntax::render(src, file, w));
                        out.push('\n');
                    }
                    out.push_str(&io.out);
                    out
                }
                Err(e) => {
                    let mut out = String::from("PANIC\n");
                    out.push_str(&io.out);
                    if !io.out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&spider_silk::render_panic(&e));
                    out
                }
            }
        }
        Err(spider_silk::PrepareError::Diagnostics(diags)) => {
            let mut out = String::from("ERR\n");
            for d in &diags {
                out.push_str(&spider_syntax::render(src, file, d));
                out.push('\n');
            }
            out
        }
        Err(spider_silk::PrepareError::Internal(m)) => {
            format!("ERR\ninternal Spider error (a bug in Spider, not your code): {m}\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playground_pipeline_end_to_end() {
        let out = run_playground("say \"Hello from wasm-land!\"\nsay 6 * 7\n", "");
        assert_eq!(out, "OK\nHello from wasm-land!\n42\n");

        let out = run_playground("let name = ask \"Who?\"\nsay \"Hi, {name}!\"\n", "Ada");
        assert_eq!(out, "OK\nHi, Ada!\n");

        let out = run_playground("say totl\n", "");
        assert!(out.starts_with("ERR\n") && out.contains("E0201"), "{out}");

        let out = run_playground("say 1 / 0\n", "");
        assert!(out.starts_with("PANIC\n") && out.contains("E0301"), "{out}");

        // Safe Mode holds in the browser.
        let out = run_playground("use files\nsay files.exists(\"x\")\n", "");
        assert!(out.starts_with("ERR\n") && out.contains("E0244"), "{out}");
    }
}
