// crates/sheaf-query/src/syntax/lexer.rs — tokenize TQL topological keywords
// FILE-030. Depends on nothing. Hand-rolled zero-dep lexer; every token
// carries its line:col span so malformed input pinpoints (METHOD-008).
// Keywords are UPPER-CASE; identifiers start alpha/underscore.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semi,
    Dot,
    Arrow, // ->
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Kw(String), // UPPER-CASE keyword, interned as given
    Eof,        // end of input with true span (trailing errors pinpoint)
}

#[derive(Clone, Debug)]
pub struct Spanned {
    pub tok: Tok,
    pub span: Span,
}

#[derive(Debug)]
pub struct LexError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

const KEYWORDS: &[&str] = &[
    "DEFINE", "CELL", "TYPE", "HYPERCELL", "MORPHISM", "USING", "PROJECTION", "AND", "MATRIX",
    "INSERT", "DISCRETE", "CONTINUOUS", "OVER", "BOUNDARY", "MATCH", "WHERE", "OR", "NOT",
    "RESTRICT", "TO", "SOLVE", "HARMONIC", "EMIT", "SECTION",
];

pub fn lex(src: &str) -> Result<Vec<Spanned>, LexError> {
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut col = 1usize;
    let bytes = src.as_bytes();
    let mut i = 0;
    let here = |l: usize, c: usize| Span { line: l, col: c };
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            ' ' | '\t' | '\r' => {
                i += 1;
                col += 1;
            }
            '\n' => {
                i += 1;
                line += 1;
                col = 1;
            }
            '-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] as char != '\n' {
                    i += 1;
                }
            }
            '(' => {
                out.push(Spanned { tok: Tok::LParen, span: here(line, col) });
                i += 1;
                col += 1;
            }
            ')' => {
                out.push(Spanned { tok: Tok::RParen, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '{' => {
                out.push(Spanned { tok: Tok::LBrace, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '}' => {
                out.push(Spanned { tok: Tok::RBrace, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '[' => {
                out.push(Spanned { tok: Tok::LBracket, span: here(line, col) });
                i += 1;
                col += 1;
            }
            ']' => {
                out.push(Spanned { tok: Tok::RBracket, span: here(line, col) });
                i += 1;
                col += 1;
            }
            ',' => {
                out.push(Spanned { tok: Tok::Comma, span: here(line, col) });
                i += 1;
                col += 1;
            }
            ':' => {
                out.push(Spanned { tok: Tok::Colon, span: here(line, col) });
                i += 1;
                col += 1;
            }
            ';' => {
                out.push(Spanned { tok: Tok::Semi, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '.' => {
                out.push(Spanned { tok: Tok::Dot, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '=' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Spanned { tok: Tok::Eq, span: here(line, col) });
                i += 2;
                col += 2;
            }
            '!' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Spanned { tok: Tok::Ne, span: here(line, col) });
                i += 2;
                col += 2;
            }
            '>' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Spanned { tok: Tok::Ge, span: here(line, col) });
                i += 2;
                col += 2;
            }
            '<' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Spanned { tok: Tok::Le, span: here(line, col) });
                i += 2;
                col += 2;
            }
            '>' => {
                out.push(Spanned { tok: Tok::Gt, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '<' => {
                out.push(Spanned { tok: Tok::Lt, span: here(line, col) });
                i += 1;
                col += 1;
            }
            '-' if i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                out.push(Spanned { tok: Tok::Arrow, span: here(line, col) });
                i += 2;
                col += 2;
            }
            '"' => {
                let start_col = col;
                i += 1;
                col += 1;
                let mut s = String::new();
                loop {
                    if i >= bytes.len() {
                        return Err(LexError { line, col, msg: "unterminated string literal".to_string() });
                    }
                    let ch = bytes[i] as char;
                    if ch == '"' {
                        i += 1;
                        col += 1;
                        break;
                    }
                    if ch == '\n' {
                        return Err(LexError { line, col, msg: "unterminated string literal".to_string() });
                    }
                    s.push(ch);
                    i += 1;
                    col += 1;
                }
                out.push(Spanned { tok: Tok::Str(s), span: here(line, start_col) });
            }
            ch if ch.is_ascii_digit() || (ch == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) => {
                let start_col = col;
                let mut j = i;
                if bytes[j] == b'-' {
                    j += 1;
                }
                while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                    j += 1;
                }
                let is_float = j < bytes.len() && bytes[j] == b'.';
                if is_float {
                    j += 1;
                    while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                        j += 1;
                    }
                }
                let text = &src[i..j];
                let tok = if is_float {
                    Tok::Float(text.parse::<f64>().map_err(|_| LexError { line, col, msg: format!("bad float literal {}", text) })?)
                } else {
                    Tok::Int(text.parse::<i64>().map_err(|_| LexError { line, col, msg: format!("bad int literal {}", text) })?)
                };
                col += j - i;
                i = j;
                out.push(Spanned { tok, span: here(line, start_col) });
            }
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let start_col = col;
                let mut j = i;
                while j < bytes.len() && ((bytes[j] as char).is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                let word = &src[i..j];
                col += j - i;
                i = j;
                let tok = if word == "true" {
                    Tok::Bool(true)
                } else if word == "false" {
                    Tok::Bool(false)
                } else if KEYWORDS.contains(&word) {
                    Tok::Kw(word.to_string())
                } else {
                    Tok::Ident(word.to_string())
                };
                out.push(Spanned { tok, span: here(line, start_col) });
            }
            '=' => {
                return Err(LexError { line, col, msg: "stray '=' (did you mean '==')?".to_string() });
            }
            '!' => {
                return Err(LexError { line, col, msg: "stray '!' (did you mean '!=')?".to_string() });
            }
            other => {
                return Err(LexError { line, col, msg: format!("unexpected character '{}'", other) });
            }
        }
    }
    out.push(Spanned { tok: Tok::Eof, span: here(line, col) });
    Ok(out)
}
