//! Minimal JSON for the LSP wire format. Hand-rolled on purpose: the
//! toolchain's zero-dependency rule (SRS NFR-1/2) extends to the server.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

pub fn parse(src: &str) -> Option<Json> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = Parser { chars, pos: 0 };
    p.skip_ws();
    let v = p.value()?;
    Some(v)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Option<Json> {
        self.skip_ws();
        match self.peek()? {
            '{' => self.object(),
            '[' => self.array(),
            '"' => Some(Json::Str(self.string()?)),
            't' => self.literal("true", Json::Bool(true)),
            'f' => self.literal("false", Json::Bool(false)),
            'n' => self.literal("null", Json::Null),
            _ => self.number(),
        }
    }

    fn literal(&mut self, word: &str, v: Json) -> Option<Json> {
        for expected in word.chars() {
            if self.bump()? != expected {
                return None;
            }
        }
        Some(v)
    }

    fn object(&mut self) -> Option<Json> {
        self.bump(); // {
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.bump();
            return Some(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            if self.bump()? != ':' {
                return None;
            }
            let val = self.value()?;
            pairs.push((key, val));
            self.skip_ws();
            match self.bump()? {
                ',' => continue,
                '}' => return Some(Json::Obj(pairs)),
                _ => return None,
            }
        }
    }

    fn array(&mut self) -> Option<Json> {
        self.bump(); // [
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.bump();
            return Some(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bump()? {
                ',' => continue,
                ']' => return Some(Json::Arr(items)),
                _ => return None,
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        if self.bump()? != '"' {
            return None;
        }
        let mut out = String::new();
        loop {
            match self.bump()? {
                '"' => return Some(out),
                '\\' => match self.bump()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'u' => {
                        let mut code = 0u32;
                        for _ in 0..4 {
                            code = code * 16 + self.bump()?.to_digit(16)?;
                        }
                        // Surrogate pairs (rare in source text): best effort.
                        if (0xD800..0xDC00).contains(&code) {
                            if self.bump() == Some('\\') && self.bump() == Some('u') {
                                let mut low = 0u32;
                                for _ in 0..4 {
                                    low = low * 16 + self.bump()?.to_digit(16)?;
                                }
                                let combined = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                out.push(char::from_u32(combined)?);
                            } else {
                                return None;
                            }
                        } else {
                            out.push(char::from_u32(code)?);
                        }
                    }
                    _ => return None,
                },
                c => out.push(c),
            }
        }
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.pos;
        while matches!(self.peek(), Some('0'..='9' | '-' | '+' | '.' | 'e' | 'E')) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<f64>().ok().map(Json::Num)
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_basics() {
        let v = parse(r#"{"jsonrpc":"2.0","id":1,"params":{"textDocument":{"uri":"file:///a.sp","text":"say \"hi\"\n"},"n":[1,2,3],"ok":true}}"#).unwrap();
        assert_eq!(v.get("id").and_then(|i| i.as_f64()), Some(1.0));
        let text = v
            .get("params")
            .and_then(|p| p.get("textDocument"))
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap();
        assert_eq!(text, "say \"hi\"\n");
        assert_eq!(escape("a\"b\nc"), "a\\\"b\\nc");
    }
}
