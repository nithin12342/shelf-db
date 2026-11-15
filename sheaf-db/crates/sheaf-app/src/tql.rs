use crate::{model::{Expr, Join, Query, Relation, Result}, sql::{self, expr}};
use sqlparser::{ast, parser::Parser, tokenizer::Token};

pub fn parse(text: &str) -> Result<Query> {
    let tokens = sql::tokens(text)?;
    sql::on_parser_stack(move || parse_tokens(tokens))
}

fn parse_tokens(tokens: Vec<Token>) -> Result<Query> {
    let mut input = Input { tokens, position: 0 };
    input.expect_word("MATCH")?;
    input.expect_word("ROWS")?;
    let from = input.relation()?;
    let mut joins = Vec::new();
    while input.word("JOIN") {
        input.expect_word("ROWS")?;
        let relation = input.relation()?;
        input.expect_word("ON")?;
        let on = expression(input.section(&["JOIN", "WHERE", "GROUP", "EMIT"], false)?, false)?;
        expr::equijoin(&on)?;
        joins.push(Join { relation, on });
    }
    sql::unique_relations(&from, &joins)?;
    let filter = if input.word("WHERE") {
        Some(expression(input.section(&["GROUP", "EMIT"], false)?, false)?)
    } else { None };
    let group_by = if input.word("GROUP") {
        input.expect_word("BY")?;
        let tokens = input.section(&["EMIT"], false)?;
        quoted_expressions(&tokens)?;
        sql::fragment(tokens, |p| p.parse_comma_separated(Parser::parse_expr))?
            .iter().map(expr::group).collect::<Result<_>>()?
    } else { Vec::new() };
    input.expect_word("EMIT")?;
    let distinct = input.word("DISTINCT");
    input.expect_word("ROWS")?;
    input.expect(Token::LBrace)?;
    let tokens = input.section(&[], true)?;
    let projection = projections(tokens)?;
    input.expect(Token::RBrace)?;
    let order_by = if input.word("ORDER") {
        input.expect_word("BY")?;
        let tokens = input.section(&["LIMIT", "OFFSET"], false)?;
        quoted_expressions(&tokens)?;
        sql::fragment(tokens, |p| p.parse_comma_separated(Parser::parse_order_by_expr))?
            .iter().map(expr::order).collect::<Result<_>>()?
    } else { Vec::new() };
    let limit = if input.word("LIMIT") { Some(input.unsigned()?) } else { None };
    let offset = if input.word("OFFSET") { input.unsigned()? } else { 0 };
    input.expect(Token::SemiColon)?;
    if input.position != input.tokens.len() { return Err("unexpected text after TQL query".into()); }
    Ok(Query { from, joins, filter, group_by, projection, order_by, limit, offset, distinct })
}

fn expression(tokens: Vec<Token>, aggregates: bool) -> Result<Expr> {
    quoted_expressions(&tokens)?;
    expr::convert(&sql::fragment(tokens, |p| p.parse_expr())?, aggregates)
}

fn quoted_expressions(tokens: &[Token]) -> Result<()> {
    for (i, token) in tokens.iter().enumerate() {
        if let Token::Word(word) = token {
            if word.quote_style == Some('"') { continue; }
            let keyword = ["TRUE", "FALSE", "NULL", "AND", "OR", "NOT", "IS", "AS", "DISTINCT",
                "ASC", "DESC", "NULLS", "FIRST", "LAST"].iter().any(|k| sql::keyword(token, k));
            let function = ["COUNT", "SUM", "AVG", "MIN", "MAX"].iter().any(|k| sql::keyword(token, k))
                && tokens.get(i + 1) == Some(&Token::LParen);
            if !keyword && !function {
                return Err("TQL identifiers require double quotes".into());
            }
        }
    }
    Ok(())
}

fn projections(tokens: Vec<Token>) -> Result<Vec<crate::model::Projection>> {
    quoted_expressions(&tokens)?;
    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    // Commas inside functions and quoted strings do not divide projection items.
    for i in 0..=tokens.len() {
        match tokens.get(i) {
            Some(Token::LParen | Token::LBracket | Token::LBrace) => depth += 1,
            Some(Token::RParen | Token::RBracket | Token::RBrace) => depth -= 1,
            Some(Token::Comma) if depth == 0 => {
                items.push(projection(&tokens[start..i])?);
                start = i + 1;
            }
            None => items.push(projection(&tokens[start..i])?),
            _ => {}
        }
    }
    Ok(items)
}

fn projection(tokens: &[Token]) -> Result<crate::model::Projection> {
    let (tokens, alias) = if tokens.len() >= 2 && sql::keyword(&tokens[tokens.len() - 2], "AS") {
        let alias = quoted_identifier(tokens.last().unwrap())?;
        (&tokens[..tokens.len() - 2], Some(alias))
    } else { (tokens, None) };
    let item = sql::fragment(tokens.to_vec(), |p| p.parse_select_item())?;
    if matches!(item, ast::SelectItem::ExprWithAlias { .. }) {
        return Err("TQL projection aliases require AS".into());
    }
    let mut result = expr::projection(&item)?;
    if alias.is_some() && matches!(&result.expr, Expr::Column { name, .. } if name == "*") {
        return Err("wildcard aliases are not supported".into());
    }
    result.alias = alias;
    Ok(result)
}

fn quoted_identifier(token: &Token) -> Result<String> {
    match token {
        Token::Word(word) if word.quote_style == Some('"') => Ok(word.value.clone()),
        _ => Err("TQL identifiers require double quotes".into()),
    }
}

struct Input {
    tokens: Vec<Token>,
    position: usize,
}

impl Input {
    fn word(&mut self, expected: &str) -> bool {
        if self.tokens.get(self.position).is_some_and(|t| sql::keyword(t, expected)) {
            self.position += 1;
            true
        } else { false }
    }

    fn expect_word(&mut self, expected: &str) -> Result<()> {
        if self.word(expected) { Ok(()) } else { Err(format!("expected {expected}")) }
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        if self.tokens.get(self.position) == Some(&expected) {
            self.position += 1;
            Ok(())
        } else { Err(format!("expected {expected}")) }
    }

    fn identifier(&mut self) -> Result<String> {
        let token = self.tokens.get(self.position).ok_or("expected quoted identifier")?;
        let value = quoted_identifier(token)?;
        self.position += 1;
        Ok(value)
    }

    fn relation(&mut self) -> Result<Relation> {
        self.expect(Token::LParen)?;
        let alias = self.identifier()?;
        self.expect(Token::Colon)?;
        let table = self.identifier()?;
        self.expect(Token::RParen)?;
        Ok(Relation { alias, table })
    }

    fn unsigned(&mut self) -> Result<usize> {
        match self.tokens.get(self.position) {
            Some(Token::Number(n, false)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                let value = n.parse().map_err(|_| "row count exceeds platform range")?;
                self.position += 1;
                Ok(value)
            }
            _ => Err("LIMIT and OFFSET require nonnegative integer literals".into()),
        }
    }

    fn section(&mut self, words: &[&str], brace: bool) -> Result<Vec<Token>> {
        let start = self.position;
        let mut depth = 0;
        while let Some(token) = self.tokens.get(self.position) {
            if depth == 0 && (token == &Token::SemiColon || (brace && token == &Token::RBrace)
                || words.iter().any(|word| sql::keyword(token, word))) { break; }
            match token {
                Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                Token::RParen | Token::RBracket | Token::RBrace => {
                    if depth == 0 { return Err("unexpected closing delimiter".into()); }
                    depth -= 1;
                }
                _ => {}
            }
            self.position += 1;
        }
        if self.position == start { return Err("expected expression".into()); }
        Ok(self.tokens[start..self.position].to_vec())
    }
}

pub fn render(query: &Query) -> String {
    let mut text = format!("MATCH ROWS {}", render_relation(&query.from));
    for join in &query.joins {
        text.push_str(&format!(" JOIN ROWS {} ON {}", render_relation(&join.relation), expr::render(&join.on)));
    }
    if let Some(filter) = &query.filter { text.push_str(&format!(" WHERE {}", expr::render(filter))); }
    if !query.group_by.is_empty() {
        text.push_str(&format!(" GROUP BY {}", query.group_by.iter().map(expr::render).collect::<Vec<_>>().join(", ")));
    }
    text.push_str(if query.distinct { " EMIT DISTINCT ROWS { " } else { " EMIT ROWS { " });
    text.push_str(&query.projection.iter().map(|p| {
        let value = expr::render(&p.expr);
        p.alias.as_ref().map_or(value.clone(), |a| format!("{value} AS {}", expr::quote(a)))
    }).collect::<Vec<_>>().join(", "));
    text.push_str(" }");
    if !query.order_by.is_empty() {
        text.push_str(" ORDER BY ");
        text.push_str(&query.order_by.iter().map(|o| format!("{} {}{}", expr::render(&o.expr),
            if o.ascending { "ASC" } else { "DESC" }, match o.nulls_first {
                None => "", Some(true) => " NULLS FIRST", Some(false) => " NULLS LAST",
            })).collect::<Vec<_>>().join(", "));
    }
    if let Some(limit) = query.limit { text.push_str(&format!(" LIMIT {limit}")); }
    if query.offset != 0 { text.push_str(&format!(" OFFSET {}", query.offset)); }
    text.push(';');
    text
}

fn render_relation(relation: &Relation) -> String {
    format!("({}: {})", expr::quote(&relation.alias), expr::quote(&relation.table))
}
