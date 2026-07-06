//! The Spider language server (Milestone M6).
//!
//! Speaks LSP over stdio: live diagnostics on open/change (the same
//! parse → resolve → infer pipeline as `spider check`), teaching hovers
//! (the Learn Mode concept database + stdlib docs), and completion
//! (keywords + stdlib members). `handle_message` is pure, so the protocol
//! is tested without a socket or an editor.

pub mod json;

use json::{escape, parse, Json};
use spider_syntax::{line_col, Diagnostic};
use std::collections::HashMap;

pub struct Server {
    docs: HashMap<String, String>,
    pub shutdown_requested: bool,
    pub exited: bool,
}

impl Server {
    pub fn new() -> Server {
        Server {
            docs: HashMap::new(),
            shutdown_requested: false,
            exited: false,
        }
    }

    /// Handles one incoming message; returns outgoing JSON payloads.
    pub fn handle_message(&mut self, raw: &str) -> Vec<String> {
        let Some(msg) = parse(raw) else {
            return Vec::new();
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").and_then(|i| i.as_f64());

        match method {
            "initialize" => {
                let id = id.unwrap_or(0.0);
                vec![format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"completionProvider":{{"triggerCharacters":["."]}}}},"serverInfo":{{"name":"spider-lsp","version":"0.1.0"}}}}}}"#
                )]
            }
            "initialized" => Vec::new(),
            "shutdown" => {
                self.shutdown_requested = true;
                let id = id.unwrap_or(0.0);
                vec![format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#)]
            }
            "exit" => {
                self.exited = true;
                Vec::new()
            }
            "textDocument/didOpen" => {
                let (Some(uri), Some(text)) = (
                    msg.get("params")
                        .and_then(|p| p.get("textDocument"))
                        .and_then(|t| t.get("uri"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string()),
                    msg.get("params")
                        .and_then(|p| p.get("textDocument"))
                        .and_then(|t| t.get("text"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string()),
                ) else {
                    return Vec::new();
                };
                self.docs.insert(uri.clone(), text);
                vec![self.diagnostics_notification(&uri)]
            }
            "textDocument/didChange" => {
                let uri = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());
                let text = msg
                    .get("params")
                    .and_then(|p| p.get("contentChanges"))
                    .and_then(|c| c.as_arr())
                    .and_then(|a| a.last())
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());
                let (Some(uri), Some(text)) = (uri, text) else {
                    return Vec::new();
                };
                self.docs.insert(uri.clone(), text);
                vec![self.diagnostics_notification(&uri)]
            }
            "textDocument/didClose" => {
                if let Some(uri) = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .and_then(|u| u.as_str())
                {
                    self.docs.remove(uri);
                }
                Vec::new()
            }
            "textDocument/hover" => {
                let id = id.unwrap_or(0.0);
                let content = self.hover(&msg).unwrap_or_default();
                if content.is_empty() {
                    vec![format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#)]
                } else {
                    vec![format!(
                        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"contents":{{"kind":"markdown","value":"{}"}}}}}}"#,
                        escape(&content)
                    )]
                }
            }
            "textDocument/completion" => {
                let id = id.unwrap_or(0.0);
                vec![format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"isIncomplete":false,"items":[{}]}}}}"#,
                    completion_items()
                )]
            }
            _ if id.is_some() => {
                // Politely decline unknown requests.
                let id = id.unwrap_or(0.0);
                vec![format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32601,"message":"not supported yet"}}}}"#
                )]
            }
            _ => Vec::new(),
        }
    }

    fn diagnostics_notification(&self, uri: &str) -> String {
        let text = self.docs.get(uri).cloned().unwrap_or_default();
        let diags = spider_hir::check_source(&text);
        let items: Vec<String> = diags.iter().map(|d| diagnostic_json(&text, d)).collect();
        format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{}","diagnostics":[{}]}}}}"#,
            escape(uri),
            items.join(",")
        )
    }

    fn hover(&self, msg: &Json) -> Option<String> {
        let params = msg.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let line = params.get("position")?.get("line")?.as_f64()? as usize;
        let character = params.get("position")?.get("character")?.as_f64()? as usize;
        let text = self.docs.get(uri)?;
        let word = word_at(text, line, character)?;
        if let Some(c) = spider_syntax::concepts::concept(&word) {
            return Some(c.to_string());
        }
        // Stdlib function or constant docs: `sqrt`, `push`, …
        for f in spider_hir::stdlib::module_fns() {
            if f.name == word {
                return Some(format!("**{}.{}** — {}", f.module, f.name, f.doc));
            }
        }
        for m in spider_hir::stdlib::method_docs() {
            if m.name == word {
                return Some(format!("**.{}** ({}) — {}", m.name, m.receiver, m.doc));
            }
        }
        None
    }
}

impl Default for Server {
    fn default() -> Self {
        Server::new()
    }
}

fn diagnostic_json(src: &str, d: &Diagnostic) -> String {
    let (l1, c1) = line_col(src, d.offset);
    let (l2, c2) = line_col(src, d.offset + d.len);
    let severity = if d.is_error() { 1 } else { 2 };
    format!(
        r#"{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"severity":{severity},"code":"{}","source":"spider","message":"{}"}}"#,
        l1 - 1,
        c1 - 1,
        l2 - 1,
        c2 - 1,
        d.code,
        escape(&d.message)
    )
}

fn word_at(text: &str, line: usize, character: usize) -> Option<String> {
    let l = text.lines().nth(line)?;
    let chars: Vec<char> = l.chars().collect();
    let mut i = character.min(chars.len());
    if i == chars.len() && i > 0 {
        i -= 1;
    }
    let is_word = |c: char| c == '_' || c.is_ascii_alphanumeric();
    if i < chars.len() && !is_word(chars[i]) && i > 0 {
        i -= 1;
    }
    if i >= chars.len() || !is_word(chars[i]) {
        return None;
    }
    let mut start = i;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = i;
    while end + 1 < chars.len() && is_word(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

fn completion_items() -> String {
    let mut items: Vec<String> = Vec::new();
    for kw in [
        "let", "var", "fn", "if", "else", "for", "in", "to", "while", "repeat", "times", "match",
        "try", "use", "return", "say", "ask", "record", "choice", "shape", "test", "public",
        "spawn", "do", "together", "and", "or", "not", "true", "false", "expect",
    ] {
        let doc = spider_syntax::concepts::concept(kw).unwrap_or("");
        items.push(format!(
            r#"{{"label":"{kw}","kind":14,"documentation":"{}"}}"#,
            escape(doc)
        ));
    }
    for f in spider_hir::stdlib::module_fns() {
        items.push(format!(
            r#"{{"label":"{}.{}","kind":3,"documentation":"{}"}}"#,
            f.module,
            f.name,
            escape(f.doc)
        ));
    }
    items.join(",")
}

/// Blocking stdio loop with Content-Length framing (the `spider lsp` entry).
pub fn run_stdio() -> i32 {
    use std::io::{Read, Write};
    let mut server = Server::new();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    loop {
        // Read headers.
        let mut header = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if input.read_exact(&mut byte).is_err() {
                return 0; // EOF
            }
            header.push(byte[0]);
            if header.ends_with(b"\r\n\r\n") {
                break;
            }
            if header.len() > 8192 {
                return 1;
            }
        }
        let header_text = String::from_utf8_lossy(&header);
        let len: usize = header_text
            .lines()
            .find_map(|l| l.strip_prefix("Content-Length:"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let mut body = vec![0u8; len];
        if input.read_exact(&mut body).is_err() {
            return 0;
        }
        let raw = String::from_utf8_lossy(&body).to_string();
        for payload in server.handle_message(&raw) {
            let _ = write!(out, "Content-Length: {}\r\n\r\n{}", payload.len(), payload);
            let _ = out.flush();
        }
        if server.exited {
            return if server.shutdown_requested { 0 } else { 1 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_advertises_capabilities() {
        let mut s = Server::new();
        let out = s.handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("hoverProvider"), "{}", out[0]);
        assert!(out[0].contains("completionProvider"), "{}", out[0]);
    }

    #[test]
    fn did_open_publishes_teaching_diagnostics() {
        let mut s = Server::new();
        let msg = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.sp","languageId":"spider","version":1,"text":"let total = 1\nsay totl\n"}}}"#;
        let out = s.handle_message(msg);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("publishDiagnostics"));
        assert!(out[0].contains("E0201"), "{}", out[0]);
        assert!(out[0].contains("did you mean `total`?"), "{}", out[0]);

        // A fix clears the diagnostics live.
        let fixed = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///a.sp","version":2},"contentChanges":[{"text":"let total = 1\nsay total\n"}]}}"#;
        let out = s.handle_message(fixed);
        assert!(out[0].contains(r#""diagnostics":[]"#), "{}", out[0]);
    }

    #[test]
    fn hover_teaches_keywords_and_stdlib() {
        let mut s = Server::new();
        s.handle_message(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.sp","text":"repeat 3 times\n    say 1\n"}}}"#,
        );
        let out = s.handle_message(
            r#"{"jsonrpc":"2.0","id":7,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///a.sp"},"position":{"line":0,"character":2}}}"#,
        );
        assert!(out[0].contains("simplest loop"), "{}", out[0]);
    }

    #[test]
    fn completion_includes_keywords_and_stdlib() {
        let mut s = Server::new();
        let out = s.handle_message(
            r#"{"jsonrpc":"2.0","id":9,"method":"textDocument/completion","params":{}}"#,
        );
        assert!(out[0].contains(r#""label":"repeat""#), "{}", out[0]);
        assert!(out[0].contains(r#""label":"math.sqrt""#), "{}", out[0]);
    }

    #[test]
    fn shutdown_exit_protocol() {
        let mut s = Server::new();
        let out = s.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#);
        assert!(out[0].contains("null"));
        s.handle_message(r#"{"jsonrpc":"2.0","method":"exit"}"#);
        assert!(s.exited && s.shutdown_requested);
    }
}
