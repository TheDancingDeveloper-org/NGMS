//! HTML and JSON selector engine for extracting values from responses.
//!
//! HTML mode uses the `scraper` crate for CSS selectors.
//! JSON mode uses `serde_json::Value` with dotted path access.

use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Selector};
use serde_json::Value as JsonValue;

use crate::definition::{FieldBlock, FilterBlock, SelectorBlock};
use crate::filters::{self, apply_filter};
use crate::template::{self, TemplateContext};

/// The parsed response document — either HTML or JSON.
#[derive(Debug, Clone)]
pub enum Document {
    Html(String),
    Json(JsonValue),
}

/// A single row context for field extraction.
#[derive(Debug, Clone)]
pub enum RowContext<'a> {
    HtmlElement(String), // Serialized outer HTML of the row element
    JsonObject(&'a JsonValue),
}

// ---------------------------------------------------------------------------
// HTML extraction
// ---------------------------------------------------------------------------

/// Check whether an HTML document contains at least one element matching the CSS selector.
pub fn html_has_selector(html: &str, selector: &str) -> bool {
    let doc = Html::parse_document(html);
    parse_selector(selector)
        .map(|sel| doc.select(&sel).next().is_some())
        .unwrap_or(false)
}

/// Extract the text content of the first element matching the CSS selector, or None.
pub fn html_select_text(html: &str, selector: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = parse_selector(selector).ok()?;
    doc.select(&sel).next().map(|el| {
        el.text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Select all row elements from an HTML document.
pub fn select_html_rows(html: &str, selector: &str, after: Option<usize>) -> Result<Vec<String>> {
    let doc = Html::parse_document(html);
    let sel = parse_selector(selector)?;

    let mut rows: Vec<String> = doc.select(&sel).map(|el| el.html()).collect();

    // Skip `after` rows from the beginning
    if let Some(skip) = after {
        if skip < rows.len() {
            rows = rows[skip..].to_vec();
        } else {
            rows.clear();
        }
    }

    Ok(rows)
}

/// Extract a field value from an HTML row fragment.
pub fn extract_html_field(
    row_html: &str,
    field: &FieldBlock,
    ctx: &TemplateContext,
) -> Result<Option<String>> {
    // If `text` is set, use template expansion (no CSS selection)
    if let Some(ref text) = field.text {
        let val = template::expand(text, ctx)?;
        return Ok(Some(apply_field_filters(&val, &field.filters, ctx)?));
    }

    let selector_str = match &field.selector {
        Some(s) => template::expand(s, ctx)?,
        None => return Ok(field.default.clone()),
    };

    let fragment = Html::parse_fragment(row_html);

    // Handle `..selector` (parent-level access for JSON rows nested via `multiple`)
    // In HTML mode, `..` means "select from the row itself"
    let effective_selector = selector_str.trim_start_matches("..");

    let sel = match parse_selector(effective_selector) {
        Ok(s) => s,
        Err(_) if field.optional.unwrap_or(false) => return Ok(field.default.clone()),
        Err(e) => return Err(e),
    };

    let element = fragment.select(&sel).next();

    match element {
        Some(el) => {
            let raw = if let Some(ref attr) = field.attribute {
                el.value().attr(attr).unwrap_or("").to_owned()
            } else {
                // If `remove` is specified, we'd need to remove child elements first
                // For now, just get the text content
                el.text().collect::<Vec<_>>().join("").trim().to_owned()
            };
            let filtered = apply_field_filters(&raw, &field.filters, ctx)?;
            if filtered.is_empty() {
                Ok(field.default.clone().or(Some(filtered)))
            } else {
                Ok(Some(filtered))
            }
        }
        None => {
            if field.optional.unwrap_or(false) {
                Ok(field.default.clone())
            } else {
                Ok(Some(String::new()))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSON extraction
// ---------------------------------------------------------------------------

/// Select all row elements from a JSON response.
pub fn select_json_rows(
    json: &JsonValue,
    selector: &str,
    sub_attribute: Option<&str>,
    multiple: bool,
) -> Result<Vec<JsonValue>> {
    let rows_val = json_path(json, selector);

    let base_rows = match rows_val {
        Some(JsonValue::Array(arr)) => arr.clone(),
        Some(val) => vec![val.clone()],
        None => return Ok(Vec::new()),
    };

    // If `attribute` and `multiple` are set, each row's sub-array becomes
    // individual rows (with parent fields accessible via `..`).
    if let Some(attr) = sub_attribute {
        if multiple {
            let mut expanded = Vec::new();
            for parent in &base_rows {
                if let Some(JsonValue::Array(children)) = json_path(parent, attr) {
                    for child in children {
                        // Merge parent into child as `__parent`
                        let mut merged = child.clone();
                        if let JsonValue::Object(ref mut map) = merged {
                            map.insert("__parent".into(), parent.clone());
                        }
                        expanded.push(merged);
                    }
                }
            }
            return Ok(expanded);
        }
    }

    Ok(base_rows)
}

/// Extract a field value from a JSON row object.
pub fn extract_json_field(
    row: &JsonValue,
    field: &FieldBlock,
    ctx: &TemplateContext,
) -> Result<Option<String>> {
    // If `text` is set, use template expansion
    if let Some(ref text) = field.text {
        let val = template::expand(text, ctx)?;
        return Ok(Some(apply_field_filters(&val, &field.filters, ctx)?));
    }

    let selector = match &field.selector {
        Some(s) => s.as_str(),
        None => return Ok(field.default.clone()),
    };

    // Handle `..field` — access parent object
    let (target, path) = if let Some(stripped) = selector.strip_prefix("..") {
        let parent = row.get("__parent").unwrap_or(row);
        (parent, stripped)
    } else {
        (row, selector)
    };

    let value = json_path(target, path);

    let raw = match value {
        Some(JsonValue::String(s)) => s.clone(),
        Some(JsonValue::Number(n)) => n.to_string(),
        Some(JsonValue::Bool(b)) => b.to_string(),
        Some(JsonValue::Null) => String::new(),
        Some(other) => other.to_string(),
        None => {
            if field.optional.unwrap_or(false) {
                return Ok(field.default.clone());
            }
            String::new()
        }
    };

    // Apply case mapping if present
    let after_case = if let Some(ref case_map) = field.case_map {
        let mut matched = None;
        for (pattern, replacement) in case_map {
            if pattern == &raw || pattern == "*" {
                matched = Some(match replacement {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    other => format!("{other:?}"),
                });
                if pattern != "*" {
                    break;
                }
            }
        }
        matched.unwrap_or(raw)
    } else {
        raw
    };

    let filtered = apply_field_filters(&after_case, &field.filters, ctx)?;
    Ok(Some(filtered))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply a chain of filters to a value.
fn apply_field_filters(
    input: &str,
    filters: &Option<Vec<FilterBlock>>,
    ctx: &TemplateContext,
) -> Result<String> {
    let mut val = input.to_owned();
    if let Some(filter_chain) = filters {
        for f in filter_chain {
            // Filter args may contain template expressions
            let resolved_args = resolve_filter_args(&f.args, ctx)?;
            val = apply_filter(&f.name, &val, &resolved_args)?;
        }
    }
    Ok(val)
}

/// Resolve template expressions within filter arguments.
fn resolve_filter_args(
    args: &crate::definition::FilterArgs,
    ctx: &TemplateContext,
) -> Result<crate::definition::FilterArgs> {
    use crate::definition::FilterArgs;

    match args {
        FilterArgs::None => Ok(FilterArgs::None),
        FilterArgs::Single(s) => {
            let expanded = template::expand(s, ctx)?;
            Ok(FilterArgs::Single(expanded))
        }
        FilterArgs::List(items) => {
            let expanded: Result<Vec<String>> =
                items.iter().map(|s| template::expand(s, ctx)).collect();
            Ok(FilterArgs::List(expanded?))
        }
    }
}

/// Navigate a dotted JSON path (e.g., `data.movies` or `$[0].id`).
fn json_path<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    let path = path.trim().trim_start_matches('$');
    if path.is_empty() {
        return Some(value);
    }
    let path = path.trim_start_matches('.');

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        // Handle array index: `[0]`
        if let Some(idx_str) = segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if let Ok(idx) = idx_str.parse::<usize>() {
                current = current.get(idx)?;
                continue;
            }
        }
        // Object key access
        current = current.get(segment)?;
    }

    Some(current)
}

/// Public JSON path accessor for use by search.rs count checks.
pub fn json_path_pub<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    json_path(value, path)
}

/// Parse a CSS selector, handling common Prowlarr patterns.
fn parse_selector(selector: &str) -> Result<Selector> {
    let selector = selector.trim();

    // Handle `$` prefix (JSON-style root selector used in some HTML defs)
    let selector = selector.trim_start_matches('$');
    if selector.is_empty() {
        // `$` alone means "all elements" — use `*`
        return Selector::parse("*")
            .map_err(|e| anyhow::anyhow!("invalid CSS selector '*': {e:?}"));
    }

    Selector::parse(selector)
        .map_err(|e| anyhow::anyhow!("invalid CSS selector '{selector}': {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_path_simple() {
        let json: JsonValue = serde_json::json!({
            "data": {
                "movies": [
                    {"title": "Ubuntu Movie", "year": 2024}
                ],
                "movie_count": 1
            }
        });

        let movies = json_path(&json, "data.movies");
        assert!(movies.is_some());
        assert!(movies.unwrap().is_array());

        let count = json_path(&json, "data.movie_count");
        assert_eq!(count, Some(&JsonValue::Number(1.into())));
    }

    #[test]
    fn json_extract_field() {
        let row: JsonValue = serde_json::json!({
            "name": "Ubuntu 24.04",
            "size": 1234567890,
            "seeders": 42
        });

        let field = FieldBlock {
            selector: Some("name".into()),
            optional: None,
            default: None,
            text: None,
            attribute: None,
            remove: None,
            filters: None,
            case_map: None,
        };

        let ctx = TemplateContext::default();
        let result = extract_json_field(&row, &field, &ctx).unwrap();
        assert_eq!(result, Some("Ubuntu 24.04".to_owned()));
    }

    #[test]
    fn json_extract_with_case_map() {
        let row: JsonValue = serde_json::json!({"quality": "1080p"});
        let mut case_map = indexmap::IndexMap::new();
        case_map.insert("720p".into(), serde_yaml::Value::Number(45.into()));
        case_map.insert("1080p".into(), serde_yaml::Value::Number(44.into()));
        case_map.insert("*".into(), serde_yaml::Value::Number(45.into()));

        let field = FieldBlock {
            selector: Some("quality".into()),
            case_map: Some(case_map),
            optional: None,
            default: None,
            text: None,
            attribute: None,
            remove: None,
            filters: None,
        };

        let ctx = TemplateContext::default();
        let result = extract_json_field(&row, &field, &ctx).unwrap();
        assert_eq!(result, Some("44".to_owned()));
    }
}
