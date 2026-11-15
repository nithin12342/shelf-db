use crate::model::{Result, MAX_REQUEST_BYTES};
use sqlparser::{dialect::GenericDialect, tokenizer::{Token, Tokenizer}};

pub(crate) fn keyword(token: &Token, expected: &str) -> bool {
    matches!(token, Token::Word(w) if w.quote_style.is_none() && w.value.eq_ignore_ascii_case(expected))
}

pub(crate) fn tokens(text: &str) -> Result<Vec<Token>> {
    if text.len() > MAX_REQUEST_BYTES {
        return Err(format!("request exceeds {MAX_REQUEST_BYTES} bytes"));
    }
    let tokens: Vec<_> = Tokenizer::new(&GenericDialect, text)
        .tokenize().map_err(|e| e.to_string())?.into_iter()
        .filter(|t| !matches!(t, Token::Whitespace(_))).collect();
    let mut delimiters = Vec::new();
    let mut operators = 0;
    let mut prefixes = 0;
    for token in &tokens {
        match token {
            Token::LParen | Token::LBracket | Token::LBrace => {
                delimiters.push(token.clone());
                if delimiters.len() > 64 {
                    return Err("nesting exceeds 64".into());
                }
            }
            Token::RParen | Token::RBracket | Token::RBrace => {
                let expected = match token {
                    Token::RParen => Token::LParen,
                    Token::RBracket => Token::LBracket,
                    _ => Token::LBrace,
                };
                if delimiters.pop() != Some(expected) {
                    return Err("unbalanced delimiters".into());
                }
            }
            _ => {}
        }
        // Bound flat operator chains too: their left-deep trees can overflow on drop.
        if matches!(token, Token::Plus | Token::Minus | Token::Mul | Token::Div
            | Token::Eq | Token::Neq | Token::Lt | Token::LtEq | Token::Gt | Token::GtEq)
            || ["AND", "OR", "NOT", "IS"].iter().any(|k| keyword(token, k))
        {
            operators += 1;
            if operators > 64 {
                return Err("expression exceeds 64 operators".into());
            }
        }
        if matches!(token, Token::Plus | Token::Minus) || keyword(token, "NOT") {
            prefixes += 1;
            if prefixes + delimiters.len() > 64 {
                return Err("nesting exceeds 64".into());
            }
        } else if !matches!(token, Token::LParen) {
            prefixes = 0;
        }
        if matches!(token, Token::Comma | Token::SemiColon)
            || ["SELECT", "FROM", "WHERE", "ON", "SET", "VALUES", "GROUP", "ORDER", "EMIT"]
                .iter().any(|k| keyword(token, k))
        {
            operators = 0;
        }
    }
    if !delimiters.is_empty() {
        return Err("unbalanced delimiters".into());
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_before_constructing_parser() {
        assert!(tokens(&"(".repeat(65)).unwrap_err().contains("nesting"));
        assert!(tokens(&"NOT ".repeat(65)).unwrap_err().contains("64"));
        assert!(tokens(&"x".repeat(MAX_REQUEST_BYTES + 1)).unwrap_err().contains("bytes"));
        assert!(tokens("SELECT '((()))', \")))\" /* ((( */ FROM t").is_ok());
    }
}
