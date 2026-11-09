// crates/sheaf-query/src/syntax/parser.rs — parse TQL into AST
// FILE-031. Depends on FILE-030 (tokens) + FILE-032 (AST shapes).
// Recursive descent; every failure carries line:col + expected/found, which
// is exactly what the malformed-input gate checks. Strict where the spec is
// strict (SOLVE requires all three params; EMIT paths need var.field form).
use super::ast::*;
use super::lexer::{Span, Spanned, Tok};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub expected: String,
    pub found: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: expected {}, found {}", self.line, self.col, self.expected, self.found)
    }
}

pub struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
}

pub(super) fn is_kw(t: &Tok, s: &str) -> bool {
    matches!(t, Tok::Kw(k) if k == s)
}

impl Parser {
    pub fn new(toks: Vec<Spanned>) -> Self {
        Self { toks, pos: 0 }
    }

    pub(super) fn peek(&self) -> Option<&Spanned> {
        self.toks.get(self.pos)
    }

    pub(super) fn span_here(&self) -> Span {
        self.peek().map(|s| s.span.clone()).unwrap_or(Span { line: 0, col: 0 })
    }

    pub(super) fn err<T>(&self, expected: &str) -> Result<T, ParseError> {
        let (found, sp) = match self.peek() {
            Some(s) => (format!("{:?}", s.tok), s.span.clone()),
            None => ("end of input".to_string(), self.span_here()),
        };
        Err(ParseError { line: sp.line, col: sp.col, expected: expected.to_string(), found })
    }

    pub(super) fn bump(&mut self) -> Option<Spanned> {
        let s = self.toks.get(self.pos).cloned();
        if s.is_some() {
            self.pos += 1;
        }
        s
    }

    pub(super) fn eat_kw(&mut self, kw: &str) -> Result<Span, ParseError> {
        match self.peek() {
            Some(s) if is_kw(&s.tok, kw) => {
                let sp = s.span.clone();
                self.bump();
                Ok(sp)
            }
            _ => self.err(&format!("keyword {}", kw)),
        }
    }

    pub(super) fn eat(&mut self, tok: &Tok, desc: &str) -> Result<Span, ParseError> {
        let ok = match (self.peek(), tok) {
            (Some(s), Tok::LParen) => matches!(s.tok, Tok::LParen),
            (Some(s), Tok::RParen) => matches!(s.tok, Tok::RParen),
            (Some(s), Tok::LBrace) => matches!(s.tok, Tok::LBrace),
            (Some(s), Tok::RBrace) => matches!(s.tok, Tok::RBrace),
            (Some(s), Tok::LBracket) => matches!(s.tok, Tok::LBracket),
            (Some(s), Tok::RBracket) => matches!(s.tok, Tok::RBracket),
            (Some(s), Tok::Comma) => matches!(s.tok, Tok::Comma),
            (Some(s), Tok::Colon) => matches!(s.tok, Tok::Colon),
            (Some(s), Tok::Semi) => matches!(s.tok, Tok::Semi),
            (Some(s), Tok::Dot) => matches!(s.tok, Tok::Dot),
            (Some(s), Tok::Arrow) => matches!(s.tok, Tok::Arrow),
            (Some(s), Tok::Eq) => matches!(s.tok, Tok::Eq),
            (Some(s), Tok::Ne) => matches!(s.tok, Tok::Ne),
            (Some(s), Tok::Gt) => matches!(s.tok, Tok::Gt),
            (Some(s), Tok::Lt) => matches!(s.tok, Tok::Lt),
            (Some(s), Tok::Ge) => matches!(s.tok, Tok::Ge),
            (Some(s), Tok::Le) => matches!(s.tok, Tok::Le),
            _ => false,
        };
        if ok {
            let sp = self.peek().unwrap().span.clone();
            self.bump();
            Ok(sp)
        } else {
            self.err(desc)
        }
    }

    pub(super) fn eat_ident(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Ident(v) => {
                    let v = v.clone();
                    self.bump();
                    Ok(v)
                }
                _ => self.err("identifier"),
            },
            None => self.err("identifier"),
        }
    }

    pub(super) fn eat_int(&mut self) -> Result<i64, ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Int(v) => {
                    let v = *v;
                    self.bump();
                    Ok(v)
                }
                _ => self.err("integer literal"),
            },
            None => self.err("integer literal"),
        }
    }

    pub(super) fn eat_number(&mut self) -> Result<f64, ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Int(v) => {
                    let v = *v as f64;
                    self.bump();
                    Ok(v)
                }
                Tok::Float(v) => {
                    let v = *v;
                    self.bump();
                    Ok(v)
                }
                _ => self.err("numeric literal"),
            },
            None => self.err("numeric literal"),
        }
    }

    pub(super) fn eat_lit(&mut self) -> Result<Lit, ParseError> {
        match self.peek() {
            Some(s) => match &s.tok {
                Tok::Int(v) => {
                    let v = *v;
                    self.bump();
                    Ok(Lit::Int(v))
                }
                Tok::Float(v) => {
                    let v = *v;
                    self.bump();
                    Ok(Lit::Float(v))
                }
                Tok::Str(v) => {
                    let v = v.clone();
                    self.bump();
                    Ok(Lit::Str(v))
                }
                Tok::Bool(v) => {
                    let v = *v;
                    self.bump();
                    Ok(Lit::Bool(v))
                }
                _ => self.err("literal (int, float, string, or bool)"),
            },
            None => self.err("literal (int, float, string, or bool)"),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(s) if matches!(s.tok, Tok::Eof) => break,
                _ => stmts.push(self.parse_stmt()?),
            }
        }
        Ok(Program { stmts })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            Some(s) if is_kw(&s.tok, "DEFINE") => self.parse_define(),
            Some(s) if is_kw(&s.tok, "INSERT") => self.parse_insert(),
            Some(s) if is_kw(&s.tok, "MATCH") => self.parse_query(),
            _ => self.err("DEFINE, INSERT, or MATCH"),
        }
    }

    fn parse_insert(&mut self) -> Result<Stmt, ParseError> {
        self.eat_kw("INSERT")?;
        match self.peek() {
            Some(s) if is_kw(&s.tok, "CELL") => {
                self.bump();
                let type_name = self.eat_ident()?;
                self.eat(&Tok::LParen, "'('")?;
                let id = self.eat_int()?;
                self.eat(&Tok::RParen, "')'")?;
                self.eat_kw("DISCRETE")?;
                let discrete = self.parse_kv_map()?;
                self.eat_kw("CONTINUOUS")?;
                let stalk = self.parse_float_list()?;
                self.eat(&Tok::Semi, "';'")?;
                Ok(Stmt::InsertCell(InsertCell { type_name, id: id.max(0) as u64, discrete, stalk }))
            }
            Some(s) if is_kw(&s.tok, "HYPERCELL") => {
                self.bump();
                let type_name = self.eat_ident()?;
                self.eat(&Tok::LParen, "'('")?;
                let id = self.eat_int()?;
                self.eat(&Tok::RParen, "')'")?;
                self.eat_kw("OVER")?;
                self.eat_kw("BOUNDARY")?;
                self.eat(&Tok::LBracket, "'['")?;
                let mut boundary = Vec::new();
                if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBracket)) {
                    loop {
                        let t = self.eat_ident()?;
                        self.eat(&Tok::LParen, "'('")?;
                        let cid = self.eat_int()?;
                        self.eat(&Tok::RParen, "')'")?;
                        boundary.push(CellRef { type_name: t, id: cid.max(0) as u64 });
                        if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                self.eat(&Tok::RBracket, "']'")?;
                self.eat_kw("DISCRETE")?;
                let discrete = self.parse_kv_map()?;
                self.eat_kw("CONTINUOUS")?;
                let stalk = self.parse_float_list()?;
                self.eat(&Tok::Semi, "';'")?;
                Ok(Stmt::InsertHyper(InsertHyper { type_name, id: id.max(0) as u64, boundary, discrete, stalk }))
            }
            _ => self.err("CELL or HYPERCELL after INSERT"),
        }
    }

    fn parse_query(&mut self) -> Result<Stmt, ParseError> {
        self.eat_kw("MATCH")?;
        self.eat_kw("HYPERCELL")?;
        let hyper_var = self.eat_ident()?;
        self.eat(&Tok::Colon, "':'")?;
        let hyper_type = self.eat_ident()?;
        self.eat_kw("OVER")?;
        self.eat(&Tok::LParen, "'('")?;
        let mut overs = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RParen)) {
            loop {
                let v = self.eat_ident()?;
                self.eat(&Tok::Colon, "':'")?;
                let t = self.eat_ident()?;
                overs.push((v, t));
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RParen, "')'")?;
        let where_clause = if matches!(self.peek().map(|s| &s.tok), Some(s) if is_kw(s, "WHERE")) {
            self.bump();
            Some(self.parse_or()?)
        } else {
            None
        };
        let restrict = if matches!(self.peek().map(|s| &s.tok), Some(s) if is_kw(s, "RESTRICT")) {
            self.bump();
            self.eat_kw("TO")?;
            self.eat_kw("BOUNDARY")?;
            self.eat(&Tok::LParen, "'('")?;
            let mut vars = Vec::new();
            if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RParen)) {
                loop {
                    vars.push(self.eat_ident()?);
                    if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.eat(&Tok::RParen, "')'")?;
            Some(vars)
        } else {
            None
        };
        self.eat_kw("SOLVE")?;
        self.eat_kw("HARMONIC")?;
        self.eat(&Tok::LParen, "'('")?;
        let mut reg: Option<f64> = None;
        let mut maxit: Option<i64> = None;
        let mut eth: Option<f64> = None;
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RParen)) {
            loop {
                let key = self.eat_ident()?;
                self.eat(&Tok::Colon, "':'")?;
                let val = self.eat_number()?;
                match key.as_str() {
                    "REGULARIZATION" => reg = Some(val),
                    "MAX_ITERATIONS" => maxit = Some(val as i64),
                    "ENERGY_THRESHOLD" => eth = Some(val),
                    _ => {
                        let sp = self.span_here();
                        return Err(ParseError {
                            line: sp.line,
                            col: sp.col,
                            expected: "REGULARIZATION, MAX_ITERATIONS, or ENERGY_THRESHOLD".to_string(),
                            found: key,
                        });
                    }
                }
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RParen, "')'")?;
        let (regularization, max_iterations, energy_threshold) = match (reg, maxit, eth) {
            (Some(r), Some(m), Some(e)) => (r, m, e),
            _ => return self.err("all three SOLVE params (REGULARIZATION, MAX_ITERATIONS, ENERGY_THRESHOLD)"),
        };
        self.eat_kw("EMIT")?;
        self.eat_kw("SECTION")?;
        self.eat(&Tok::LBrace, "'{'")?;
        let mut emit = Vec::new();
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::RBrace)) {
            loop {
                match self.peek() {
                    Some(s) => match &s.tok {
                        Tok::Ident(v) if v == "SECTION_ENERGY" => {
                            self.bump();
                            emit.push(EmitItem::SectionEnergy);
                        }
                        Tok::Ident(v) if v == "COHOMOLOGY_STATE" => {
                            self.bump();
                            emit.push(EmitItem::CohomologyState);
                        }
                        _ => emit.push(EmitItem::Field(self.parse_path()?)),
                    },
                    None => return self.err("emit item"),
                }
                if matches!(self.peek().map(|s| &s.tok), Some(Tok::Comma)) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RBrace, "'}'")?;
        self.eat(&Tok::Semi, "';'")?;
        Ok(Stmt::Query(Query {
            hyper_var,
            hyper_type,
            overs,
            where_clause,
            restrict,
            solve: SolveParams { regularization, max_iterations: max_iterations.max(0) as usize, energy_threshold },
            emit,
        }))
    }

    pub(super) fn parse_path(&mut self) -> Result<Vec<String>, ParseError> {
        let mut parts = vec![self.eat_ident()?];
        if !matches!(self.peek().map(|s| &s.tok), Some(Tok::Dot)) {
            return self.err("dotted path like p.status");
        }
        while matches!(self.peek().map(|s| &s.tok), Some(Tok::Dot)) {
            self.bump();
            parts.push(self.eat_ident()?);
        }
        Ok(parts)
    }

}

/// One-call frontend: lex + parse with pinpoint errors as strings.
pub fn parse_program(src: &str) -> Result<Program, String> {
    let toks = super::lexer::lex(src).map_err(|e| format!("{}:{}: {}", e.line, e.col, e.msg))?;
    Parser::new(toks).parse_program().map_err(|e| e.to_string())
}
