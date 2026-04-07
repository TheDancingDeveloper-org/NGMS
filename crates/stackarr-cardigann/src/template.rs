//! Go-template-like expression evaluator for Cardigann YAML paths and values.
//!
//! Supports the subset of Go templates actually used across Prowlarr's 549
//! Cardigann definitions: variable interpolation, conditionals, range, and
//! a small set of functions (join, or, eq, and, not).

use std::collections::HashMap;

use anyhow::{Result, bail};

/// Context variables available during template expansion.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    /// User settings + `sitelink` (the active base URL).
    pub config: HashMap<String, String>,
    /// The search keywords (already filtered by `keywordsfilters`).
    pub keywords: String,
    /// Category IDs to search (as strings).
    pub categories: Vec<String>,
    /// Search query parameters.
    pub query: QueryContext,
    /// Previously-extracted field values (for cross-references like `.Result._id`).
    pub result: HashMap<String, String>,
    /// Boolean constants.
    pub true_val: String,
    pub false_val: String,
}

/// Search query parameters.
#[derive(Debug, Clone, Default)]
pub struct QueryContext {
    pub imdbid: String,
    pub tvdbid: String,
    pub tmdbid: String,
    pub season: String,
    pub ep: String,
    pub album: String,
    pub artist: String,
    pub label: String,
    pub track: String,
    pub year: String,
}

impl TemplateContext {
    /// Resolve a dotted variable path like `.Config.sitelink` or `.Keywords`.
    pub fn resolve(&self, path: &str) -> String {
        let path = path.trim().trim_start_matches('.');
        let parts: Vec<&str> = path.splitn(2, '.').collect();

        match parts[0].to_lowercase().as_str() {
            "config" if parts.len() == 2 => self.config.get(parts[1]).cloned().unwrap_or_default(),
            "keywords" => self.keywords.clone(),
            "categories" => self.categories.join(","),
            "query" if parts.len() == 2 => self.resolve_query(parts[1]),
            "result" if parts.len() == 2 => self.result.get(parts[1]).cloned().unwrap_or_default(),
            "true" => "true".to_owned(),
            "false" => "false".to_owned(),
            "today" if parts.len() == 2 => {
                let now = chrono::Utc::now();
                match parts[1].to_lowercase().as_str() {
                    "year" => now.format("%Y").to_string(),
                    "month" => now.format("%m").to_string(),
                    "day" => now.format("%d").to_string(),
                    _ => String::new(),
                }
            }
            _ => String::new(),
        }
    }

    fn resolve_query(&self, field: &str) -> String {
        match field.to_uppercase().as_str() {
            "IMDBID" => self.query.imdbid.clone(),
            "TVDBID" => self.query.tvdbid.clone(),
            "TMDBID" => self.query.tmdbid.clone(),
            "SEASON" => self.query.season.clone(),
            "EP" | "EPISODE" => self.query.ep.clone(),
            "ALBUM" => self.query.album.clone(),
            "ARTIST" => self.query.artist.clone(),
            "LABEL" => self.query.label.clone(),
            "TRACK" => self.query.track.clone(),
            "YEAR" => self.query.year.clone(),
            _ => String::new(),
        }
    }

    /// Check if a variable is "truthy" (non-empty, not "false", not "0").
    pub fn is_truthy(&self, path: &str) -> bool {
        let val = self.resolve(path);
        !val.is_empty() && val != "false" && val != "0"
    }
}

/// Expand a Go-template string against the given context.
///
/// Handles: `{{ .Var }}`, `{{ if .X }}...{{ else }}...{{ end }}`,
/// `{{ range .Categories }}...{{ end }}`, `{{ join .X "," }}`,
/// `{{ or .A .B }}`, `{{ eq .A "lit" }}`, `{{ and .A .B }}`.
pub fn expand(template: &str, ctx: &TemplateContext) -> Result<String> {
    let mut output = String::with_capacity(template.len());
    let mut pos = 0;
    while pos < template.len() {
        if let Some(start) = template[pos..].find("{{") {
            // Append literal text before the tag
            output.push_str(&template[pos..pos + start]);
            let tag_start = pos + start + 2;

            // Find matching }}
            let end = template[tag_start..]
                .find("}}")
                .map(|e| tag_start + e)
                .ok_or_else(|| anyhow::anyhow!("unclosed template tag"))?;

            let tag = template[tag_start..end].trim();
            pos = end + 2;

            // Handle different tag types
            if let Some(rest) = tag.strip_prefix("if ") {
                let (if_body, else_body, new_pos) = parse_if_block(template, pos)?;
                pos = new_pos;
                let condition = evaluate_condition(rest.trim(), ctx);
                if condition {
                    output.push_str(&expand(&if_body, ctx)?);
                } else if let Some(eb) = else_body {
                    output.push_str(&expand(&eb, ctx)?);
                }
            } else if let Some(rest) = tag.strip_prefix("range ") {
                let (body, new_pos) = parse_range_block(template, pos)?;
                pos = new_pos;
                let var_path = rest.trim();
                let items = resolve_iterable(var_path, ctx);
                for item in &items {
                    let item_template = body.replace("{{ . }}", item).replace("{{.}}", item);
                    output.push_str(&expand(&item_template, ctx)?);
                }
            } else if let Some(rest) = tag.strip_prefix("join ") {
                let (var, sep) = parse_join_args(rest)?;
                let items = resolve_iterable(&var, ctx);
                output.push_str(&items.join(&sep));
            } else if let Some(rest) = tag.strip_prefix("or ") {
                let result = evaluate_or(rest, ctx);
                output.push_str(&result);
            } else if let Some(rest) = tag.strip_prefix("eq ") {
                let result = evaluate_eq(rest, ctx);
                output.push_str(if result { "true" } else { "false" });
            } else if let Some(rest) = tag.strip_prefix("and ") {
                let result = evaluate_and(rest, ctx);
                output.push_str(if result { "true" } else { "" });
            } else if let Some(rest) = tag.strip_prefix("not ") {
                let val = ctx.resolve(rest.trim());
                output.push_str(if val.is_empty() || val == "false" || val == "0" {
                    "true"
                } else {
                    ""
                });
            } else if tag == "else" || tag == "end" {
                // Should have been consumed by if/range parsing
                // If we hit this, there's a nesting error — just skip
            } else {
                // Simple variable interpolation: {{ .Config.sitelink }}
                let val = ctx.resolve(tag);
                output.push_str(&val);
            }
        } else {
            // No more tags — append rest of string
            output.push_str(&template[pos..]);
            break;
        }
    }

    Ok(output)
}

/// Parse an `if` block, returning (if_body, optional else_body, position after `{{ end }}`).
fn parse_if_block(template: &str, start: usize) -> Result<(String, Option<String>, usize)> {
    let mut depth = 1;
    let mut pos = start;
    let mut else_pos = None;

    while pos < template.len() && depth > 0 {
        if let Some(tag_start) = template[pos..].find("{{") {
            let abs_start = pos + tag_start + 2;
            if let Some(tag_end) = template[abs_start..].find("}}") {
                let abs_end = abs_start + tag_end;
                let tag = template[abs_start..abs_end].trim();

                if tag.starts_with("if ") {
                    depth += 1;
                } else if tag == "end" {
                    depth -= 1;
                    if depth == 0 {
                        let if_body = if let Some(ep) = else_pos {
                            template[start..ep].to_owned()
                        } else {
                            template[start..pos + tag_start].to_owned()
                        };
                        let else_body = else_pos.map(|ep| {
                            // ep points to start of {{ else }}, skip past it
                            let after_else = template[ep..].find("}}").unwrap() + ep + 2;
                            template[after_else..pos + tag_start].to_owned()
                        });
                        return Ok((if_body, else_body, abs_end + 2));
                    }
                } else if tag == "else" && depth == 1 {
                    else_pos = Some(pos + tag_start);
                }

                pos = abs_end + 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    bail!("unterminated if block")
}

/// Parse a `range` block, returning (body, position after `{{ end }}`).
fn parse_range_block(template: &str, start: usize) -> Result<(String, usize)> {
    let mut depth = 1;
    let mut pos = start;

    while pos < template.len() && depth > 0 {
        if let Some(tag_start) = template[pos..].find("{{") {
            let abs_start = pos + tag_start + 2;
            if let Some(tag_end) = template[abs_start..].find("}}") {
                let abs_end = abs_start + tag_end;
                let tag = template[abs_start..abs_end].trim();

                if tag.starts_with("range ") || tag.starts_with("if ") {
                    depth += 1;
                } else if tag == "end" {
                    depth -= 1;
                    if depth == 0 {
                        let body = template[start..pos + tag_start].to_owned();
                        return Ok((body, abs_end + 2));
                    }
                }
                pos = abs_end + 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    bail!("unterminated range block")
}

/// Evaluate a condition expression for `{{ if ... }}`.
fn evaluate_condition(expr: &str, ctx: &TemplateContext) -> bool {
    let expr = expr.trim();

    // {{ if and .A .B }}
    if let Some(rest) = expr.strip_prefix("and ") {
        return evaluate_and(rest, ctx);
    }
    // {{ if or .A .B }}
    if let Some(rest) = expr.strip_prefix("or ") {
        let result = evaluate_or(rest, ctx);
        return !result.is_empty() && result != "false" && result != "0";
    }
    // {{ if eq .A "literal" }}
    if let Some(rest) = expr.strip_prefix("eq ") {
        return evaluate_eq(rest, ctx);
    }
    // {{ if not .A }}
    if let Some(rest) = expr.strip_prefix("not ") {
        return !ctx.is_truthy(rest.trim());
    }
    // Simple variable truthiness
    ctx.is_truthy(expr)
}

/// Evaluate `and .A .B` — returns true if all args are truthy.
fn evaluate_and(args_str: &str, ctx: &TemplateContext) -> bool {
    // Handle nested function calls: and (.A) (eq .B "val")
    let args = parse_function_args(args_str);
    args.iter().all(|arg| {
        let arg = arg.trim().trim_start_matches('(').trim_end_matches(')');
        evaluate_condition(arg, ctx)
    })
}

/// Evaluate `or .A .B` — returns first truthy value (as a string).
fn evaluate_or(args_str: &str, ctx: &TemplateContext) -> String {
    let args = parse_function_args(args_str);
    for arg in &args {
        let arg = arg.trim().trim_start_matches('(').trim_end_matches(')');
        let val = ctx.resolve(arg);
        if !val.is_empty() && val != "false" && val != "0" {
            return val;
        }
    }
    String::new()
}

/// Evaluate `eq .A "literal"` or `eq .A .B`.
fn evaluate_eq(args_str: &str, ctx: &TemplateContext) -> bool {
    let args = parse_function_args(args_str);
    if args.len() < 2 {
        return false;
    }
    let left = resolve_or_literal(&args[0], ctx);
    let right = resolve_or_literal(&args[1], ctx);
    left == right
}

/// Parse space-separated arguments, respecting quoted strings and parenthesised groups.
fn parse_function_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';
    let mut paren_depth = 0;

    for ch in input.chars() {
        match ch {
            '"' | '\'' if paren_depth == 0 => {
                // Keep quotes in the output so resolve_or_literal can detect literals
                current.push(ch);
                if in_quotes && ch == quote_char {
                    in_quotes = false;
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                }
            }
            '(' if !in_quotes => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' if !in_quotes => {
                paren_depth -= 1;
                current.push(ch);
            }
            ' ' if !in_quotes && paren_depth == 0 => {
                let trimmed = current.trim().to_owned();
                if !trimmed.is_empty() {
                    args.push(trimmed);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let trimmed = current.trim().to_owned();
    if !trimmed.is_empty() {
        args.push(trimmed);
    }

    args
}

/// Resolve a value that could be a dotted variable path or a string literal.
fn resolve_or_literal(val: &str, ctx: &TemplateContext) -> String {
    let val = val.trim();
    // Quoted string literal
    if (val.starts_with('"') && val.ends_with('"'))
        || (val.starts_with('\'') && val.ends_with('\''))
    {
        return val[1..val.len() - 1].to_owned();
    }
    // Parenthesised expression
    if val.starts_with('(') && val.ends_with(')') {
        let inner = &val[1..val.len() - 1];
        if let Some(rest) = inner.strip_prefix("eq ") {
            return if evaluate_eq(rest, ctx) {
                "true".to_owned()
            } else {
                "false".to_owned()
            };
        }
        return ctx.resolve(inner);
    }
    // Variable reference
    ctx.resolve(val)
}

/// Parse `join .Categories ","` arguments.
fn parse_join_args(input: &str) -> Result<(String, String)> {
    let args = parse_function_args(input);
    if args.len() < 2 {
        bail!("join requires a variable and separator");
    }
    let var = args[0].clone();
    let sep = args[1].trim_matches('"').trim_matches('\'').to_owned();
    Ok((var, sep))
}

/// Resolve a variable path to a list of items for `range` or `join`.
fn resolve_iterable(path: &str, ctx: &TemplateContext) -> Vec<String> {
    let path = path.trim().trim_start_matches('.');
    match path.to_lowercase().as_str() {
        "categories" => ctx.categories.clone(),
        _ => {
            let val = ctx.resolve(path);
            if val.is_empty() {
                Vec::new()
            } else {
                vec![val]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> TemplateContext {
        let mut ctx = TemplateContext::default();
        ctx.config
            .insert("sitelink".into(), "https://example.com/".into());
        ctx.config.insert("apiurl".into(), "apibay.org".into());
        ctx.keywords = "ubuntu".into();
        ctx.categories = vec!["100".into(), "200".into()];
        ctx.query.imdbid = "tt1234567".into();
        ctx.result.insert("_id".into(), "42".into());
        ctx
    }

    #[test]
    fn simple_variable() {
        let ctx = test_ctx();
        let result = expand("{{ .Keywords }}", &ctx).unwrap();
        assert_eq!(result, "ubuntu");
    }

    #[test]
    fn config_variable() {
        let ctx = test_ctx();
        let result = expand("https://{{ .Config.apiurl }}/search", &ctx).unwrap();
        assert_eq!(result, "https://apibay.org/search");
    }

    #[test]
    fn if_else() {
        let ctx = test_ctx();
        let result = expand("{{ if .Keywords }}has_kw{{ else }}no_kw{{ end }}", &ctx).unwrap();
        assert_eq!(result, "has_kw");

        let mut ctx2 = test_ctx();
        ctx2.keywords = String::new();
        let result = expand("{{ if .Keywords }}has_kw{{ else }}no_kw{{ end }}", &ctx2).unwrap();
        assert_eq!(result, "no_kw");
    }

    #[test]
    fn if_with_query() {
        let ctx = test_ctx();
        let tpl = "{{ if .Query.IMDBID }}{{ .Query.IMDBID }}{{ else }}{{ .Keywords }}{{ end }}";
        let result = expand(tpl, &ctx).unwrap();
        assert_eq!(result, "tt1234567");
    }

    #[test]
    fn join_categories() {
        let ctx = test_ctx();
        let result = expand("{{ join .Categories \",\" }}", &ctx).unwrap();
        assert_eq!(result, "100,200");
    }

    #[test]
    fn result_reference() {
        let ctx = test_ctx();
        let result = expand(
            "{{ .Config.sitelink }}description.php?id={{ .Result._id }}",
            &ctx,
        )
        .unwrap();
        assert_eq!(result, "https://example.com/description.php?id=42");
    }

    #[test]
    fn eq_function() {
        let mut ctx = test_ctx();
        ctx.result.insert("_type".into(), "web".into());
        let tpl = "{{ if eq .Result._type \"web\" }}WEBRip{{ else }}BRRip{{ end }}";
        let result = expand(tpl, &ctx).unwrap();
        assert_eq!(result, "WEBRip");
    }
}
