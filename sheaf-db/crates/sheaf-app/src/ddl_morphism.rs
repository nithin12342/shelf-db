// Timestamp: 2026-11-22_11-53-08 +05:30
// ddl_morphism.rs - the morphism declaration DDL parser (NF-15, S-02).
//
// The schema guide declares a morphism from an existing lower-dimensional cell
// to an existing higher-dimensional cell:
//
//   DEFINE MORPHISM AccountToPayment
//     FROM Account TO Payment
//     PROJ (account_id -> tx_id)
//     MATRIX [[1.0, 0.0], [0.0, 1.0]];
//
// This parser accepts that grammar only. It uses a strict token set and returns
// a named error for every wrong shape. It never guesses a column or a number.
// The Database validates that the two cells and every projected column exist.

use crate::model::{MorphismDecl, Result};

/// One lexical item of a morphism declaration.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Arrow,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
}

fn lex(source: &str) -> Result<Vec<Token>> {
    if source.len() > crate::model::MAX_REQUEST_BYTES {
        return Err("morphism declaration exceeds 65536 bytes".into());
    }
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        match byte {
            b'(' => { out.push(Token::LParen); at += 1; }
            b')' => { out.push(Token::RParen); at += 1; }
            b'[' => { out.push(Token::LBracket); at += 1; }
            b']' => { out.push(Token::RBracket); at += 1; }
            b',' => { out.push(Token::Comma); at += 1; }
            b';' => { out.push(Token::Semi); at += 1; }
            b'-' if bytes.get(at + 1) == Some(&b'>') => { out.push(Token::Arrow); at += 2; }
            b'-' | b'0'..=b'9' => {
                let start = at;
                if byte == b'-' { at += 1; }
                while bytes.get(at).is_some_and(u8::is_ascii_digit) { at += 1; }
                if bytes.get(at) == Some(&b'.') {
                    at += 1;
                    while bytes.get(at).is_some_and(u8::is_ascii_digit) { at += 1; }
                }
                let text = std::str::from_utf8(&bytes[start..at]).map_err(|_| "invalid number")?;
                let value: f64 = text.parse().map_err(|_| "invalid number")?;
                if !value.is_finite() { return Err("morphism matrix requires finite numbers".into()); }
                out.push(Token::Number(value));
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = at;
                while bytes.get(at).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') { at += 1; }
                let text = std::str::from_utf8(&bytes[start..at]).map_err(|_| "invalid identifier")?;
                out.push(Token::Ident(text.into()));
            }
            _ => return Err(format!("unexpected character in morphism declaration: {}", byte as char)),
        }
    }
    Ok(out)
}

/// Whether the source starts with `DEFINE MORPHISM`.
pub(crate) fn is_declaration(tokens: &[sqlparser::tokenizer::Token]) -> bool {
    use sqlparser::tokenizer::Token;
    fn word(token: Option<&Token>, expected: &str) -> bool {
        matches!(token, Some(Token::Word(w)) if w.quote_style.is_none() && w.value.eq_ignore_ascii_case(expected))
    }
    word(tokens.first(), "DEFINE") && word(tokens.get(1), "MORPHISM")
}

/// Parse one morphism declaration.
pub fn parse(source: &str) -> Result<MorphismDecl> {
    let tokens = lex(source)?;
    Parser { tokens: &tokens, at: 0 }.parse()
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
}

impl Parser<'_> {
    fn keyword(&mut self, word: &str) -> Result<()> {
        match self.tokens.get(self.at) {
            Some(Token::Ident(found)) if found.eq_ignore_ascii_case(word) => {
                self.at += 1;
                Ok(())
            }
            _ => Err(format!("expected {word} in morphism declaration")),
        }
    }

    fn ident(&mut self) -> Result<String> {
        match self.tokens.get(self.at) {
            Some(Token::Ident(name)) => {
                self.at += 1;
                Ok(name.clone())
            }
            _ => Err("expected an identifier in morphism declaration".into()),
        }
    }

    fn eat(&mut self, token: Token) -> Result<()> {
        if self.tokens.get(self.at) == Some(&token) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {token:?} in morphism declaration"))
        }
    }

    fn parse(mut self) -> Result<MorphismDecl> {
        self.keyword("DEFINE")?;
        self.keyword("MORPHISM")?;
        let name = self.ident()?;
        self.keyword("FROM")?;
        let from = self.ident()?;
        self.keyword("TO")?;
        let to = self.ident()?;
        self.keyword("PROJ")?;
        self.eat(Token::LParen)?;
        let mut projection = Vec::new();
        loop {
            let source_column = self.ident()?;
            self.eat(Token::Arrow)?;
            let target_column = self.ident()?;
            projection.push((source_column, target_column));
            if self.tokens.get(self.at) == Some(&Token::Comma) {
                self.at += 1;
                continue;
            }
            break;
        }
        self.eat(Token::RParen)?;
        self.keyword("MATRIX")?;
        self.eat(Token::LBracket)?;
        let mut matrix: Vec<Vec<f64>> = Vec::new();
        loop {
            self.eat(Token::LBracket)?;
            let mut row = Vec::new();
            loop {
                match self.tokens.get(self.at) {
                    Some(Token::Number(value)) => {
                        row.push(*value);
                        self.at += 1;
                    }
                    Some(Token::Comma) => self.at += 1,
                    _ => break,
                }
            }
            self.eat(Token::RBracket)?;
            matrix.push(row);
            if self.tokens.get(self.at) == Some(&Token::Comma) {
                self.at += 1;
                continue;
            }
            break;
        }
        self.eat(Token::RBracket)?;
        self.eat(Token::Semi)?;
        if self.at != self.tokens.len() {
            return Err("trailing tokens after morphism declaration".into());
        }
        if projection.is_empty() {
            return Err("a morphism requires at least one projection".into());
        }
        if matrix.is_empty() || matrix[0].is_empty() {
            return Err("a morphism requires a nonempty matrix".into());
        }
        let width = matrix[0].len();
        if matrix.iter().any(|row| row.len() != width) {
            return Err("morphism matrix rows must have equal width".into());
        }
        Ok(MorphismDecl { name, from, to, projection, matrix })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declaration_from_the_schema_guide_parses() {
        let text = "DEFINE MORPHISM AccountToPayment FROM Account TO Payment PROJ (account_id -> tx_id) MATRIX [[1.0, 0.0], [0.0, 1.0]];";
        let decl = parse(text).expect("parse");
        assert_eq!(decl.name, "AccountToPayment");
        assert_eq!(decl.from, "Account");
        assert_eq!(decl.to, "Payment");
        assert_eq!(decl.projection, vec![("account_id".into(), "tx_id".into())]);
        assert_eq!(decl.matrix, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    }

    #[test]
    fn missing_parts_are_named() {
        let text = "DEFINE MORPHISM M FROM A TO B PROJ (a -> b) MATRIX [[1.0]]";
        assert_eq!(parse(text).err().as_deref(), Some("expected Semi in morphism declaration"));
        let text = "DEFINE MORPHISM M FROM A TO B PROJ () MATRIX [[1.0]];";
        assert_eq!(parse(text).err().as_deref(), Some("expected an identifier in morphism declaration"));
    }

    #[test]
    fn a_ragged_matrix_is_rejected() {
        let text = "DEFINE MORPHISM M FROM A TO B PROJ (a -> b) MATRIX [[1.0, 2.0], [3.0]];";
        assert_eq!(parse(text).err().as_deref(), Some("morphism matrix rows must have equal width"));
    }
}
