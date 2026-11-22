// C-10: procedural macros that specialize the native tiers at compile time.
// Timestamp: 2026-11-22_12-52-18 +05:30
// Remote: origin https://github.com/nithin12342/shelf-db.git
// Branch: combined-stack-feature-improvement
//
// The two attributes below run in the compiler. They emit ordinary Rust
// tokens. The [sheaf_unroll] attribute keeps the annotated function unchanged
// so that runtime results do not change. The [rule_table] attribute reads the
// emitted C header with include_str! and builds a static table from it.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, token, Ident, ItemConst, ItemFn, LitInt, Result, Visibility};

/// Absolute path to the emitted header, resolved when this crate compiles.
/// The path is also emitted into consumer code for its include_str! check.
const RULE_HEADER_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../native/proofs-generated/sheaf_verified_rules.generated.h"
);

/// The emitted header, read at compile time with include_str!.
const RULE_HEADER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../native/proofs-generated/sheaf_verified_rules.generated.h"
));

/// Argument list for `#[sheaf_unroll(lanes = N)]`.
struct UnrollArgs {
    lanes: usize,
}

impl Parse for UnrollArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: Ident = input.parse()?;
        if key != "lanes" {
            return Err(syn::Error::new_spanned(key, "expected `lanes`"));
        }
        let _equals: token::Eq = input.parse()?;
        let value: LitInt = input.parse()?;
        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "unexpected tokens after lanes value"));
        }
        Ok(UnrollArgs {
            lanes: value.base10_parse::<usize>()?,
        })
    }
}

/// Emit compile-time lane metadata for a function that walks a slice.
///
/// The annotated function is emitted byte for byte, so its runtime result is
/// identical to the unannotated version. The generated module is a proof that
/// the compiler ran this macro for that function.
#[proc_macro_attribute]
pub fn sheaf_unroll(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as UnrollArgs);
    let function = parse_macro_input!(item as ItemFn);
    let function_name = function.sig.ident.clone();
    let metadata_module = format_ident!("{}_sheaf_unroll", function_name);
    let lanes = args.lanes;

    quote! {
        #function

        #[doc = "Compile-time lane metadata emitted by sheaf_unroll."]
        pub mod #metadata_module {
            pub const LANES: usize = #lanes;
            pub const APPLIED: bool = true;

            pub const fn lane_count() -> usize {
                LANES
            }
        }
    }
    .into()
}

/// One row of the generated rule header.
struct Rule {
    id: u16,
    name: String,
    theorem: String,
}

/// Return the text inside the first pair of double quotes in `text`.
fn quoted_value(text: &str) -> &str {
    let bytes = text.as_bytes();
    let open = bytes.iter().position(|byte| *byte == b'"').unwrap_or(0);
    let tail = &bytes[(open + 1).min(bytes.len())..];
    let close = tail.iter().position(|byte| *byte == b'"').unwrap_or(tail.len());
    &text[(open + 1).min(text.len())..(open + 1 + close).min(text.len())]
}

/// Return the text that follows the first pair of double quotes in `text`.
fn after_quoted_value(text: &str) -> &str {
    let bytes = text.as_bytes();
    let open = bytes.iter().position(|byte| *byte == b'"').unwrap_or(0);
    let tail = &bytes[(open + 1).min(bytes.len())..];
    let close = tail.iter().position(|byte| *byte == b'"').unwrap_or(tail.len());
    &text[(open + close + 2).min(text.len())..]
}

/// Parse the six `{ id, "name", "theorem" }` rows of the emitted header.
/// The C struct declaration is skipped because no digit follows its brace.
fn parse_rules(header: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for line in header.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let tail = line[1..].trim_start();
        if !tail.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            continue;
        }
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        let id = digits.parse::<u16>().unwrap_or(0);
        let name = quoted_value(tail).to_string();
        let theorem = quoted_value(after_quoted_value(tail)).to_string();
        rules.push(Rule { id, name, theorem });
    }
    rules
}

/// Replace a const declaration with a static rule table built from the header.
///
/// The macro reads the header with include_str! while it expands. It parses
/// the six rule ids and names at that time. The generated code has no runtime
/// parsing step. A compile-time assertion checks the header length seen by
/// the consumer against the length seen by this macro.
#[proc_macro_attribute]
pub fn rule_table(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(Span::call_site(), "rule_table takes no arguments")
            .to_compile_error()
            .into();
    }

    let declaration = parse_macro_input!(item as ItemConst);
    let table_name = declaration.ident;
    let visibility: Visibility = declaration.vis;
    let rule_module = format_ident!("__sheaf_rule_table_{}", table_name);

    let rules = parse_rules(RULE_HEADER);
    if rules.len() != 6 {
        let message = format!("expected 6 rules in the emitted header, found {}", rules.len());
        return syn::Error::new(Span::call_site(), message)
            .to_compile_error()
            .into();
    }

    let ids: Vec<u16> = rules.iter().map(|rule| rule.id).collect();
    let names: Vec<String> = rules.iter().map(|rule| rule.name.clone()).collect();
    let theorems: Vec<String> = rules.iter().map(|rule| rule.theorem.clone()).collect();
    let header_len = RULE_HEADER.len();
    let header_path = RULE_HEADER_PATH;

    quote! {
        #[allow(non_snake_case)]
        mod #rule_module {
            #[doc = "One verified rule from the emitted C header."]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub struct SheafRule {
                pub id: u16,
                pub name: &'static str,
                pub theorem: &'static str,
            }

            pub const RULE_COUNT: usize = 6usize;

            // include_str! embeds the emitted header at the consumer's
            // compile time. The length check ties the table below to it.
            const HEADER: &str = include_str!(#header_path);
            pub const HEADER_BYTES: usize = #header_len;
            const _: () = assert!(HEADER.len() == HEADER_BYTES);

            pub static TABLE: [SheafRule; RULE_COUNT] = [
                #(
                    SheafRule {
                        id: #ids,
                        name: #names,
                        theorem: #theorems,
                    }
                ),*
            ];
        }

        #visibility use #rule_module::{SheafRule, TABLE as #table_name};
    }
    .into()
}
