//! Prototype: determine the completion context within a jq query.
//!
//! Splits a query into (base_expr, segment) at the last *top-level* pipe,
//! ignoring pipes inside string literals and parentheses. The `base_expr`
//! is fed to run_jaq; the `segment` is the trailing path being completed.
//!
//! Segmentation uses jaq's own tokenizer (`jaq_core::load::lex`) rather than a
//! hand-rolled byte scanner, so it can never disagree with how jq parses the
//! query. The lexer already handles string literals/escapes/interpolation
//! (`Tok::Str`) and groups each `(...)`/`[...]`/`{...}` into one recursive
//! `Tok::Block`; a `|` nested inside any of those is simply not a top-level
//! token. The `|=` update-assignment lexes as a distinct `Tok::Sym` (`"|="`),
//! so excluding it is free.

use jaq_core::load::lex::{Lexer, Tok};

#[derive(Debug, PartialEq, Eq)]
pub struct CompletionCtx {
    /// Expression before the last top-level pipe (to evaluate). Empty => root.
    pub base: String,
    /// Byte offset where the segment-to-complete begins (after pipe + whitespace).
    pub segment_start: usize,
}

/// Find the byte offset of the last top-level `|` pipe symbol (not `|=`),
/// using jaq's lexer.
///
/// Returns `None` when there is no top-level pipe, or when the query fails to
/// lex (e.g. unbalanced delimiters during mid-typing like `select(.a`); callers
/// then degrade to the root context. This is never worse than the old byte
/// scanner, which "succeeded" on such input but produced an invalid `base` that
/// downstream evaluation rejected into empty suggestions anyway.
fn last_top_level_pipe(query: &str) -> Option<usize> {
    // Walk only the top-level token vec; never descend into `Tok::Block`.
    let tokens = Lexer::new(query).lex().ok()?;
    let last = tokens
        .iter()
        .rfind(|tok| matches!(tok.1, Tok::Sym) && tok.0 == "|")?;
    // Recover the slice's byte offset within `query` (the same computation as
    // jaq's private `load::span`, inlined to avoid depending on its visibility).
    Some(last.0.as_ptr() as usize - query.as_ptr() as usize)
}

pub fn analyze(query: &str) -> CompletionCtx {
    match last_top_level_pipe(query) {
        Some(p) => {
            let base = query[..p].trim().to_string();
            // segment begins after the pipe, skipping whitespace
            let after = p + 1;
            let ws = query[after..].len() - query[after..].trim_start().len();
            CompletionCtx {
                base,
                segment_start: after + ws,
            }
        }
        None => {
            // no pipe: whole query is the segment (skip leading ws)
            let ws = query.len() - query.trim_start().len();
            CompletionCtx {
                base: String::new(),
                segment_start: ws,
            }
        }
    }
}

/// Result of cursor-aware segmentation.
#[derive(Debug, PartialEq, Eq)]
pub struct Segments {
    /// Expression to evaluate (empty => root).
    pub base: String,
    /// Text left of the cursor, up to the start of the segment being completed.
    pub preserved_prefix: String,
    /// The token being completed (left of the cursor, after the segment start).
    pub segment: String,
    /// Text right of the cursor, preserved verbatim.
    pub suffix: String,
}

/// Segment a query for completion at byte offset `cursor` (must be on a char
/// boundary — guaranteed by the char→byte conversion at the call site).
/// Analyzes only the text left of the cursor; the right side is preserved.
///
/// Cursor-at-end (`cursor == query.len()`, the common case) makes `left ==
/// query` and `suffix == ""`, so behavior is byte-identical to analyzing the
/// whole query — regression-safe.
pub fn segment_at_cursor(query: &str, cursor: usize) -> Segments {
    let left = &query[..cursor];
    let suffix = query[cursor..].to_string();
    let ctx = analyze(left); // token-tree segmentation, unchanged
    Segments {
        base: ctx.base,
        preserved_prefix: left[..ctx.segment_start].to_string(),
        segment: left[ctx.segment_start..].to_string(),
        suffix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg<'a>(q: &'a str, ctx: &CompletionCtx) -> &'a str {
        &q[ctx.segment_start..]
    }

    #[test]
    fn table() {
        let cases = [
            // (query, expected base, expected segment)
            (".foo | .ba", ".foo", ".ba"),
            (".foo|.ba", ".foo", ".ba"),
            (".", "", "."),
            (".foo.ba", "", ".foo.ba"),
            (".a | .b | .c", ".a | .b", ".c"),
            ("(.a | .b) | .c", "(.a | .b)", ".c"),
            (".items[] | .na", ".items[]", ".na"),
            (r#".foo["a|b"] | .c"#, r#".foo["a|b"]"#, ".c"),
            ("{a: .x | .y} | .z", "{a: .x | .y}", ".z"), // pipe inside object value
            (".foo | ", ".foo", ""),
            (".foo |= .bar", "", ".foo |= .bar"), // |= not a boundary
            ("  .foo", "", ".foo"),               // leading ws
            ("map(.a) | .b", "map(.a)", ".b"),
            // token-tree specifics:
            (".a.", "", ".a."), // open trailing dot lexes cleanly (`.a`, `.`)
            ("select(.a", "", "select(.a"), // unbalanced: degrades to root, no panic
            (r#""\(.a|.b)" | .c"#, r#""\(.a|.b)""#, ".c"), // pipe inside string interpolation
        ];
        let mut failures = vec![];
        for (q, want_base, want_seg) in cases {
            let ctx = analyze(q);
            let got_seg = seg(q, &ctx);
            if ctx.base != want_base || got_seg != want_seg {
                failures.push(format!(
                    "query {q:?}: got base={:?} seg={:?}, want base={:?} seg={:?}",
                    ctx.base, got_seg, want_base, want_seg
                ));
            }
        }
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        assert!(
            failures.is_empty(),
            "{} segmentation cases failed",
            failures.len()
        );
    }

    #[test]
    fn cursor_table() {
        // (query, cursor byte offset, expected base, expected segment, expected suffix)
        let cases = [
            // cursor at end ≡ analyzing the whole query (regression proof)
            (".foo | .ba", 10, ".foo", ".ba", ""),
            // mid-line within the first token: suffix preserved verbatim
            (".foo | .ba", 4, "", ".foo", " | .ba"),
            // cursor mid-pipe: only the left segment is analyzed
            (".a | .b | .c", 7, ".a", ".b", " | .c"),
            // cursor at 0 / empty query
            ("", 0, "", "", ""),
            (".foo", 0, "", "", ".foo"),
            // multi-byte suffix preserved (boundary-safe slice)
            (".a | .b café", 7, ".a", ".b", " café"),
        ];
        let mut failures = vec![];
        for (q, cursor, want_base, want_seg, want_suffix) in cases {
            let seg = segment_at_cursor(q, cursor);
            if seg.base != want_base || seg.segment != want_seg || seg.suffix != want_suffix {
                failures.push(format!(
                    "query {q:?} @ {cursor}: got base={:?} seg={:?} suffix={:?}, \
                     want base={:?} seg={:?} suffix={:?}",
                    seg.base, seg.segment, seg.suffix, want_base, want_seg, want_suffix
                ));
            }
        }
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        assert!(
            failures.is_empty(),
            "{} cursor segmentation cases failed",
            failures.len()
        );
    }
}
