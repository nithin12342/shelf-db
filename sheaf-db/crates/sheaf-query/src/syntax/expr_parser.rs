// crates/sheaf-query/src/syntax/expr_parser.rs — parse TQL WHERE expressions
// FILE-031b. Depends on FILE-030 (tokens) + FILE-032 (AST shapes).
// Extracted from parser.rs to satisfy SPEC-022 (≤500 lines per file).
use super::ast::*;
use super::lexer::Tok;
use super::parser::{is_kw, ParseError, Parser};

impl Parser {
    pub(super) fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().map(|s| &s.tok), Some(s) if is_kw(s, "OR")) {
            self.bump();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    pub(super) fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        while matches!(self.peek().map(|s| &s.tok), Some(s) if is_kw(s, "AND")) {
            self.bump();
            let right = self.parse_unary()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    pub(super) fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek().map(|s| &s.tok), Some(s) if is_kw(s, "NOT")) {
            self.bump();
            return Ok(Expr::Not(Box::new(self.parse_unary()?)));
        }
        if matches!(self.peek().map(|s| &s.tok), Some(Tok::LParen)) {
            self.bump();
            let e = self.parse_or()?;
            self.eat(&Tok::RParen, "')'")?;
            return Ok(e);
        }
        let path = self.parse_path()?;
        let op = match self.peek() {
            Some(s) => match &s.tok {
                Tok::Eq => CmpOp::Eq,
                Tok::Ne => CmpOp::Ne,
                Tok::Gt => CmpOp::Gt,
                Tok::Lt => CmpOp::Lt,
                Tok::Ge => CmpOp::Ge,
                Tok::Le => CmpOp::Le,
                _ => return self.err("comparison operator (==, !=, >, <, >=, <=)"),
            },
            None => return self.err("comparison operator (==, !=, >, <, >=, <=)"),
        };
        self.bump();
        let lit = self.eat_lit()?;
        Ok(Expr::Cmp { path, op, lit })
    }
}
