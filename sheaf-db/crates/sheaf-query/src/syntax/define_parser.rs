// crates/sheaf-query/src/syntax/define_parser.rs — parse TQL DEFINE statements
// FILE-031c. Depends on FILE-030 (tokens) + FILE-032 (AST shapes).
// Extracted from parser.rs to satisfy SPEC-022 (≤500 lines per file).
use super::ast::{CellDef, Lit, MorphDef, Stmt};
use super::lexer::Tok;
use super::parser::{is_kw, ParseError, Parser};

impl Parser {
    pub(super) fn parse_define(&mut self) -> Result<Stmt, ParseError> {
        self.eat_kw("DEFINE")?;
        match self.peek() {
            Some(s) if is_kw(&s.tok, "CELL") || is_kw(&s.tok, "HYPERCELL") => self.parse_define_cell(),
            Some(s) if is_kw(&s.tok, "MORPHISM") => self.parse_define_morphism(),
            _ => self.err("CELL, HYPERCELL, or MORPHISM after DEFINE"),
        }
    }

    fn parse_define_cell(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            Some(s) if is_kw(&s.tok, "CELL") || is_kw(&s.tok, "HYPERCELL") => {
                self.bump();
            }
            _ => return self.err("CELL or HYPERCELL"),
        }
        self.eat_kw("TYPE")?;
        let name = self.eat_ident()?;
        self.eat(&Tok::LParen, "'('")?;
        self.eat_ident_named("dim")?;
        self.eat(&Tok::Colon, "':'")?;
        let dim = self.eat_int()?;
        if dim < 0 {
            let sp = self.span_here();
            return Err(ParseError { line: sp.line, col: sp.col, expected: "non-negative dim".to_string(), found: dim.to_string() });
        }
        self.eat(&Tok::Comma, "','")?;
        self.eat_ident_named("discrete")?;
        self.eat(&Tok::Colon, "':'")?;
        self.eat(&Tok::LBrace, "'{'")?;
        let mut fields = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBrace)) {
            loop {
                let f = self.eat_ident()?;
                self.eat(&Tok::Colon, "':'")?;
                self.eat_ident()?; // field type name; types checked downstream
                fields.push(f);
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBrace, "'}'")?;
        self.eat(&Tok::Comma, "','")?;
        self.eat_ident_named("continuous")?;
        self.eat(&Tok::Colon, "':'")?;
        self.eat_ident_named("Float32")?;
        self.eat(&Tok::LBracket, "'['")?;
        let cont_dim = self.eat_int()?;
        self.eat(&Tok::RBracket, "']'")?;
        self.eat(&Tok::RParen, "')'")?;
        self.eat(&Tok::Semi, "';'")?;
        Ok(Stmt::DefineCell(CellDef { name, dim: dim as u8, discrete_fields: fields, cont_dim: cont_dim.max(0) as usize }))
    }

    fn eat_ident_named(&mut self, want: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Ident(v) if v == want => {
                    self.bump();
                    Ok(())
                }
                _ => self.err(&format!("identifier '{}'", want)),
            },
            None => self.err(&format!("identifier '{}'", want)),
        }
    }

    /// Like `eat_ident_named` but also accepts a UPPER-CASE keyword of the
    /// same spelling (grammar words like PROJECTION/MATRIX arrive as Tok::Kw).
    fn eat_word_named(&mut self, want: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Ident(v) if v == want => {
                    self.bump();
                    Ok(())
                }
                Tok::Kw(v) if v == want => {
                    self.bump();
                    Ok(())
                }
                _ => self.err(&format!("'{}'", want)),
            },
            None => self.err(&format!("'{}'", want)),
        }
    }

    fn parse_define_morphism(&mut self) -> Result<Stmt, ParseError> {
        self.eat_kw("MORPHISM")?;
        let name = self.eat_ident()?;
        self.eat(&Tok::Colon, "':'")?;
        let from_type = self.eat_ident()?;
        self.eat(&Tok::Arrow, "'->'")?;
        let to_type = self.eat_ident()?;
        self.eat_kw("USING")?;
        self.eat_word_named("PROJECTION")?;
        self.eat(&Tok::LBrace, "'{'")?;
        let mut proj = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBrace)) {
            loop {
                let a = self.eat_ident()?;
                self.eat(&Tok::Arrow, "'->'")?;
                let b = self.eat_ident()?;
                proj.push((a, b));
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBrace, "'}'")?;
        self.eat_kw("AND")?;
        self.eat_word_named("MATRIX")?;
        self.eat(&Tok::LBracket, "'['")?;
        let mut matrix = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBracket)) {
            loop {
                self.eat(&Tok::LBracket, "'['")?;
                let mut row = Vec::new();
                if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBracket)) {
                    loop {
                        row.push(self.eat_number()?);
                        if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                self.eat(&Tok::RBracket, "']'")?;
                matrix.push(row);
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBracket, "']'")?;
        self.eat(&Tok::Semi, "';'")?;
        Ok(Stmt::DefineMorphism(MorphDef { name, from_type, to_type, proj_fields: proj, matrix }))
    }

    pub(super) fn parse_kv_map(&mut self) -> Result<Vec<(String, Lit)>, ParseError> {
        self.eat(&Tok::LBrace, "'{'")?;
        let mut out = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBrace)) {
            loop {
                let k = self.eat_ident()?;
                self.eat(&Tok::Colon, "':'")?;
                let v = self.eat_lit()?;
                out.push((k, v));
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBrace, "'}'")?;
        Ok(out)
    }

    pub(super) fn parse_float_list(&mut self) -> Result<Vec<f32>, ParseError> {
        self.eat(&Tok::LBracket, "'['")?;
        let mut out = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBracket)) {
            loop {
                out.push(self.eat_number()? as f32);
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBracket, "']'")?;
        Ok(out)
    }
}
