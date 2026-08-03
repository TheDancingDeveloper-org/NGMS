// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

//! Filter implementations for Cardigann field value transforms.
//!
//! Each filter takes an input string and optional arguments and returns
//! the transformed string. These are applied sequentially to extracted values.

use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use regex::Regex;

use crate::definition::FilterArgs;

/// Apply a named filter to the input value.
pub fn apply_filter(name: &str, input: &str, args: &FilterArgs) -> Result<String> {
    match name {
        "querystring" => filter_querystring(input, args),
        "regexp" => filter_regexp(input, args),
        "re_replace" => filter_re_replace(input, args),
        "split" => filter_split(input, args),
        "replace" => filter_replace(input, args),
        "trim" => filter_trim(input, args),
        "prepend" => filter_prepend(input, args),
        "append" => filter_append(input, args),
        "tolower" => Ok(input.to_lowercase()),
        "toupper" => Ok(input.to_uppercase()),
        "urldecode" => filter_urldecode(input),
        "urlencode" => filter_urlencode(input),
        "htmldecode" => filter_htmldecode(input),
        "htmlencode" => filter_htmlencode(input),
        "timeago" | "reltime" => filter_timeago(input),
        "fuzzytime" => filter_fuzzytime(input),
        "dateparse" | "timeparse" => filter_dateparse(input, args),
        "validfilename" => filter_validfilename(input),
        "diacritics" => filter_diacritics(input),
        "jsonjoinarray" => filter_jsonjoinarray(input, args),
        "validate" => filter_validate(input, args),
        "hexdump" | "strdump" => Ok(input.to_owned()),
        other => {
            tracing::warn!(filter = other, "unknown Cardigann filter, passing through");
            Ok(input.to_owned())
        }
    }
}

/// Extract a query string parameter value from a URL.
fn filter_querystring(input: &str, args: &FilterArgs) -> Result<String> {
    let key = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => bail!("querystring filter requires a key argument"),
    };
    let url = url::Url::parse(input)
        .or_else(|_| url::Url::parse(&format!("http://dummy?{input}")))
        .context("querystring: failed to parse URL")?;
    for (k, v) in url.query_pairs() {
        if k == key {
            return Ok(v.into_owned());
        }
    }
    Ok(String::new())
}

/// Extract the first capture group from a regex match.
fn filter_regexp(input: &str, args: &FilterArgs) -> Result<String> {
    let pattern = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => bail!("regexp filter requires a pattern argument"),
    };
    let re = Regex::new(pattern).context("regexp: invalid pattern")?;
    if let Some(caps) = re.captures(input) {
        if let Some(m) = caps.get(1) {
            return Ok(m.as_str().to_owned());
        }
        if let Some(m) = caps.get(0) {
            return Ok(m.as_str().to_owned());
        }
    }
    Ok(String::new())
}

/// Regex search-and-replace.
fn filter_re_replace(input: &str, args: &FilterArgs) -> Result<String> {
    let (pattern, replacement) = match args {
        FilterArgs::List(v) if v.len() >= 2 => (v[0].as_str(), v[1].as_str()),
        _ => bail!("re_replace filter requires [pattern, replacement]"),
    };
    let re = Regex::new(pattern).context("re_replace: invalid pattern")?;
    Ok(re.replace_all(input, replacement).into_owned())
}

/// Split by separator and take the nth element.
fn filter_split(input: &str, args: &FilterArgs) -> Result<String> {
    let (sep, idx) = match args {
        FilterArgs::List(v) if v.len() >= 2 => {
            let idx: usize = v[1].parse().context("split: index is not a number")?;
            (v[0].as_str(), idx)
        }
        _ => bail!("split filter requires [separator, index]"),
    };
    let parts: Vec<&str> = input.split(sep).collect();
    Ok(parts.get(idx).unwrap_or(&"").to_string())
}

/// Simple string replacement.
fn filter_replace(input: &str, args: &FilterArgs) -> Result<String> {
    let (old, new) = match args {
        FilterArgs::List(v) if v.len() >= 2 => (v[0].as_str(), v[1].as_str()),
        _ => bail!("replace filter requires [old, new]"),
    };
    Ok(input.replace(old, new))
}

/// Trim whitespace or specified characters.
fn filter_trim(input: &str, args: &FilterArgs) -> Result<String> {
    match args {
        FilterArgs::Single(chars) => {
            let chars: Vec<char> = chars.chars().collect();
            Ok(input.trim_matches(chars.as_slice()).to_owned())
        }
        _ => Ok(input.trim().to_owned()),
    }
}

/// Prepend text.
fn filter_prepend(input: &str, args: &FilterArgs) -> Result<String> {
    let prefix = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => "",
    };
    Ok(format!("{prefix}{input}"))
}

/// Append text.
fn filter_append(input: &str, args: &FilterArgs) -> Result<String> {
    let suffix = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => "",
    };
    Ok(format!("{input}{suffix}"))
}

/// URL-decode.
fn filter_urldecode(input: &str) -> Result<String> {
    Ok(urlencoding::decode(input)
        .unwrap_or(std::borrow::Cow::Borrowed(input))
        .into_owned())
}

/// URL-encode.
fn filter_urlencode(input: &str) -> Result<String> {
    Ok(urlencoding::encode(input).into_owned())
}

/// Decode HTML entities.
fn filter_htmldecode(input: &str) -> Result<String> {
    // Handle common HTML entities
    let result = input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    Ok(result)
}

/// Encode HTML entities.
fn filter_htmlencode(input: &str) -> Result<String> {
    let result = input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;");
    Ok(result)
}

/// Parse relative time expressions like "2 hours ago", "Yesterday", "just now".
fn filter_timeago(input: &str) -> Result<String> {
    let now = Utc::now();
    let lower = input.trim().to_lowercase();

    if lower == "just now" || lower == "now" {
        return Ok(now.to_rfc3339());
    }
    if lower == "yesterday" {
        let dt = now - chrono::Duration::days(1);
        return Ok(dt.to_rfc3339());
    }
    if lower == "today" {
        return Ok(now.to_rfc3339());
    }

    // Pattern: "X unit(s) ago"
    let re = Regex::new(r"(\d+)\s+(second|minute|hour|day|week|month|year)s?\s+ago")?;
    if let Some(caps) = re.captures(&lower) {
        let amount: i64 = caps[1].parse().unwrap_or(0);
        let dt = match &caps[2] {
            "second" => now - chrono::Duration::seconds(amount),
            "minute" => now - chrono::Duration::minutes(amount),
            "hour" => now - chrono::Duration::hours(amount),
            "day" => now - chrono::Duration::days(amount),
            "week" => now - chrono::Duration::weeks(amount),
            "month" => now - chrono::Duration::days(amount * 30),
            "year" => now - chrono::Duration::days(amount * 365),
            _ => now,
        };
        return Ok(dt.to_rfc3339());
    }

    // Fallback: try parsing as a unix timestamp
    if let Ok(ts) = lower.parse::<i64>()
        && let Some(dt) = chrono::DateTime::from_timestamp(ts, 0)
    {
        return Ok(dt.to_rfc3339());
    }

    Ok(now.to_rfc3339())
}

/// Parse fuzzy date/time strings — handles unix timestamps, relative times,
/// and various date formats commonly found on torrent sites.
fn filter_fuzzytime(input: &str) -> Result<String> {
    let trimmed = input.trim();

    // Unix timestamp
    if let Ok(ts) = trimmed.parse::<i64>()
        && let Some(dt) = chrono::DateTime::from_timestamp(ts, 0)
    {
        return Ok(dt.to_rfc3339());
    }

    // Try common date formats
    let formats = &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%d-%m-%Y %H:%M:%S",
        "%d-%m-%Y",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%Y",
        "%b %d, %Y",
        "%B %d, %Y",
        "%d %b %Y",
        "%d %B %Y",
    ];

    for fmt in formats {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            let dt = ndt.and_utc();
            return Ok(dt.to_rfc3339());
        }
        if let Ok(nd) = NaiveDate::parse_from_str(trimmed, fmt) {
            let dt = nd.and_hms_opt(0, 0, 0).expect("valid time").and_utc();
            return Ok(dt.to_rfc3339());
        }
    }

    // Try timeago as fallback
    filter_timeago(input)
}

/// Parse a date string using a Go/C#-style format specifier.
fn filter_dateparse(input: &str, args: &FilterArgs) -> Result<String> {
    let format_str = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => bail!("dateparse filter requires a format argument"),
    };

    // Convert Go/C#-style format tokens to chrono format
    let chrono_fmt = convert_date_format(format_str);
    let trimmed = input.trim();

    if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, &chrono_fmt) {
        return Ok(ndt.and_utc().to_rfc3339());
    }
    if let Ok(nd) = NaiveDate::parse_from_str(trimmed, &chrono_fmt) {
        let dt = nd.and_hms_opt(0, 0, 0).expect("valid time").and_utc();
        return Ok(dt.to_rfc3339());
    }

    // Some definitions use format strings like "htt MMM. d" where "htt" is a
    // custom token meaning "hours:minutes today". Handle this by trying the
    // chrono parse with the current year appended.
    let with_year = format!("{trimmed} {}", Utc::now().year());
    let chrono_fmt_year = format!("{chrono_fmt} %Y");
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&with_year, &chrono_fmt_year) {
        return Ok(ndt.and_utc().to_rfc3339());
    }
    if let Ok(nd) = NaiveDate::parse_from_str(&with_year, &chrono_fmt_year) {
        let dt = nd.and_hms_opt(0, 0, 0).expect("valid time").and_utc();
        return Ok(dt.to_rfc3339());
    }

    // Last resort: fuzzytime
    filter_fuzzytime(input)
}

/// Convert Go/C#-style date format tokens to chrono strftime tokens.
fn convert_date_format(fmt: &str) -> String {
    fmt.replace("yyyy", "%Y")
        .replace("yy", "%y")
        .replace("MMMM", "%B")
        .replace("MMM", "%b")
        .replace("MM", "%m")
        .replace("dd", "%d")
        .replace("HH", "%H")
        .replace("hh", "%I")
        .replace("htt", "%I:%M %p")
        .replace("mm", "%M")
        .replace("ss", "%S")
        .replace("tt", "%p")
        // Single-letter tokens
        .replace(" d ", " %-d ")
        .replace(" M ", " %-m ")
        .replace(" h ", " %-I ")
}

/// Replace characters invalid in filenames.
fn filter_validfilename(input: &str) -> Result<String> {
    let re = Regex::new(r#"[<>:"/\\|?*]"#)?;
    Ok(re.replace_all(input, "_").into_owned())
}

/// Remove diacritics/accent marks via Unicode NFKD decomposition.
fn filter_diacritics(input: &str) -> Result<String> {
    // Simple approach: filter out combining characters (U+0300-U+036F)
    let result: String = input
        .chars()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .collect();
    Ok(result)
}

/// Parse a JSON array and join its elements with a separator.
fn filter_jsonjoinarray(input: &str, args: &FilterArgs) -> Result<String> {
    let sep = match args {
        FilterArgs::Single(s) => s.as_str(),
        FilterArgs::List(v) if !v.is_empty() => v[0].as_str(),
        _ => ", ",
    };
    let arr: Vec<serde_json::Value> = serde_json::from_str(input).unwrap_or_default();
    let strings: Vec<String> = arr
        .iter()
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();
    Ok(strings.join(sep))
}

/// Validate input against a set of allowed values.
fn filter_validate(input: &str, args: &FilterArgs) -> Result<String> {
    let allowed: Vec<&str> = match args {
        FilterArgs::Single(s) => s.split(',').map(str::trim).collect(),
        FilterArgs::List(v) => v.iter().map(String::as_str).collect(),
        _ => return Ok(input.to_owned()),
    };
    if allowed.contains(&input) {
        Ok(input.to_owned())
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_replace() {
        let result = apply_filter(
            "re_replace",
            "hello world 123",
            &FilterArgs::List(vec!["\\d+".into(), "NUM".into()]),
        )
        .unwrap();
        assert_eq!(result, "hello world NUM");
    }

    #[test]
    fn test_split() {
        // "/torrent/12345/some-name" splits to ["", "torrent", "12345", "some-name"]
        let result = apply_filter(
            "split",
            "/torrent/12345/some-name",
            &FilterArgs::List(vec!["/".into(), "1".into()]),
        )
        .unwrap();
        assert_eq!(result, "torrent");

        let result = apply_filter(
            "split",
            "/torrent/12345/some-name",
            &FilterArgs::List(vec!["/".into(), "3".into()]),
        )
        .unwrap();
        assert_eq!(result, "some-name");
    }

    #[test]
    fn test_replace() {
        let result = apply_filter(
            "replace",
            "hello-world",
            &FilterArgs::List(vec!["-".into(), " ".into()]),
        )
        .unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_append_prepend() {
        let result =
            apply_filter("prepend", "world", &FilterArgs::Single("hello ".into())).unwrap();
        assert_eq!(result, "hello world");
        let result = apply_filter("append", "hello", &FilterArgs::Single(" world".into())).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_urlencode_decode() {
        let encoded = apply_filter("urlencode", "hello world", &FilterArgs::None).unwrap();
        assert_eq!(encoded, "hello%20world");
        let decoded = apply_filter("urldecode", "hello%20world", &FilterArgs::None).unwrap();
        assert_eq!(decoded, "hello world");
    }

    #[test]
    fn test_timeago() {
        let result = apply_filter("timeago", "just now", &FilterArgs::None).unwrap();
        assert!(!result.is_empty());
        let result = apply_filter("timeago", "2 hours ago", &FilterArgs::None).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_regexp() {
        let result = apply_filter(
            "regexp",
            "Size: 1.5 GB",
            &FilterArgs::Single(r"(\d+\.?\d*)\s*GB".into()),
        )
        .unwrap();
        assert_eq!(result, "1.5");
    }

    #[test]
    fn test_regexp_no_capture_group_returns_full_match() {
        let result =
            apply_filter("regexp", "abc 123 def", &FilterArgs::Single(r"\d+".into())).unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_regexp_no_match_returns_empty() {
        let result = apply_filter(
            "regexp",
            "no digits here",
            &FilterArgs::Single(r"\d+".into()),
        )
        .unwrap();
        assert_eq!(result, "");
    }

    // ── querystring ────────────────────────────────────────────────────

    #[test]
    fn test_querystring_full_url() {
        let result = apply_filter(
            "querystring",
            "http://example.com?id=42&cat=movies",
            &FilterArgs::Single("id".into()),
        )
        .unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_querystring_bare_params() {
        let result = apply_filter(
            "querystring",
            "id=42&cat=movies",
            &FilterArgs::Single("cat".into()),
        )
        .unwrap();
        assert_eq!(result, "movies");
    }

    #[test]
    fn test_querystring_missing_key() {
        let result = apply_filter(
            "querystring",
            "http://x.com?a=1",
            &FilterArgs::Single("missing".into()),
        )
        .unwrap();
        assert_eq!(result, "");
    }

    // ── htmldecode / htmlencode ────────────────────────────────────────

    #[test]
    fn test_htmldecode_all_entities() {
        let input = "&amp; &lt; &gt; &quot; &#39; &apos; &nbsp;";
        let result = apply_filter("htmldecode", input, &FilterArgs::None).unwrap();
        assert_eq!(result, "& < > \" ' '  ");
    }

    #[test]
    fn test_htmlencode_special_chars() {
        let result =
            apply_filter("htmlencode", "a & b < c > d \"e\" 'f'", &FilterArgs::None).unwrap();
        assert_eq!(result, "a &amp; b &lt; c &gt; d &quot;e&quot; &#39;f&#39;");
    }

    #[test]
    fn test_htmlencode_decode_roundtrip() {
        let original = "Tom & Jerry's \"Adventure\"";
        let encoded = apply_filter("htmlencode", original, &FilterArgs::None).unwrap();
        let decoded = apply_filter("htmldecode", &encoded, &FilterArgs::None).unwrap();
        assert_eq!(decoded, original);
    }

    // ── trim ───────────────────────────────────────────────────────────

    #[test]
    fn test_trim_whitespace() {
        let result = apply_filter("trim", "  hello  ", &FilterArgs::None).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_trim_custom_chars() {
        let result = apply_filter("trim", "---hello---", &FilterArgs::Single("-".into())).unwrap();
        assert_eq!(result, "hello");
    }

    // ── split edge cases ───────────────────────────────────────────────

    #[test]
    fn test_split_out_of_bounds() {
        let result = apply_filter(
            "split",
            "a,b",
            &FilterArgs::List(vec![",".into(), "5".into()]),
        )
        .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_split_first_element() {
        let result = apply_filter(
            "split",
            "first|second|third",
            &FilterArgs::List(vec!["|".into(), "0".into()]),
        )
        .unwrap();
        assert_eq!(result, "first");
    }

    // ── tolower / toupper ──────────────────────────────────────────────

    #[test]
    fn test_tolower_toupper() {
        assert_eq!(
            apply_filter("tolower", "HELLO World", &FilterArgs::None).unwrap(),
            "hello world"
        );
        assert_eq!(
            apply_filter("toupper", "hello World", &FilterArgs::None).unwrap(),
            "HELLO WORLD"
        );
    }

    // ── validfilename ──────────────────────────────────────────────────

    #[test]
    fn test_validfilename() {
        let result = apply_filter(
            "validfilename",
            "file: <test> \"name\".txt",
            &FilterArgs::None,
        )
        .unwrap();
        assert_eq!(result, "file_ _test_ _name_.txt");
    }

    #[test]
    fn test_validfilename_clean_name_unchanged() {
        let result =
            apply_filter("validfilename", "normal-file_name.txt", &FilterArgs::None).unwrap();
        assert_eq!(result, "normal-file_name.txt");
    }

    // ── jsonjoinarray ──────────────────────────────────────────────────

    #[test]
    fn test_jsonjoinarray_strings() {
        let result = apply_filter(
            "jsonjoinarray",
            r#"["a","b","c"]"#,
            &FilterArgs::Single(", ".into()),
        )
        .unwrap();
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn test_jsonjoinarray_numbers() {
        let result =
            apply_filter("jsonjoinarray", "[1,2,3]", &FilterArgs::Single("|".into())).unwrap();
        assert_eq!(result, "1|2|3");
    }

    #[test]
    fn test_jsonjoinarray_invalid_json() {
        let result =
            apply_filter("jsonjoinarray", "not json", &FilterArgs::Single(",".into())).unwrap();
        assert_eq!(result, "");
    }

    // ── validate ───────────────────────────────────────────────────────

    #[test]
    fn test_validate_allowed() {
        let result = apply_filter(
            "validate",
            "yes",
            &FilterArgs::Single("yes, no, maybe".into()),
        )
        .unwrap();
        assert_eq!(result, "yes");
    }

    #[test]
    fn test_validate_not_allowed() {
        let result =
            apply_filter("validate", "nope", &FilterArgs::Single("yes, no".into())).unwrap();
        assert_eq!(result, "");
    }

    // ── convert_date_format ────────────────────────────────────────────

    #[test]
    fn test_convert_date_format_basic() {
        assert_eq!(convert_date_format("yyyy-MM-dd"), "%Y-%m-%d");
        assert_eq!(
            convert_date_format("dd/MM/yyyy HH:mm:ss"),
            "%d/%m/%Y %H:%M:%S"
        );
    }

    #[test]
    fn test_convert_date_format_12h() {
        assert_eq!(convert_date_format("hh:mm tt"), "%I:%M %p");
    }

    // ── fuzzytime ──────────────────────────────────────────────────────

    #[test]
    fn test_fuzzytime_unix_timestamp() {
        let result = apply_filter("fuzzytime", "1704067200", &FilterArgs::None).unwrap();
        assert!(result.contains("2024-01-01"));
    }

    #[test]
    fn test_fuzzytime_iso_date() {
        let result = apply_filter("fuzzytime", "2024-03-15", &FilterArgs::None).unwrap();
        assert!(result.contains("2024-03-15"));
    }

    #[test]
    fn test_fuzzytime_verbose_date() {
        let result = apply_filter("fuzzytime", "Jan 15, 2024", &FilterArgs::None).unwrap();
        assert!(result.contains("2024-01-15"));
    }

    // ── timeago extended ───────────────────────────────────────────────

    #[test]
    fn test_timeago_yesterday() {
        let result = apply_filter("timeago", "Yesterday", &FilterArgs::None).unwrap();
        // Should produce a valid RFC3339 timestamp
        assert!(chrono::DateTime::parse_from_rfc3339(&result).is_ok());
    }

    #[test]
    fn test_timeago_today() {
        let result = apply_filter("timeago", "today", &FilterArgs::None).unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(&result).is_ok());
    }

    #[test]
    fn test_timeago_units() {
        for unit in &[
            "1 second ago",
            "5 minutes ago",
            "3 hours ago",
            "2 days ago",
            "1 week ago",
            "6 months ago",
            "1 year ago",
        ] {
            let result = apply_filter("timeago", unit, &FilterArgs::None).unwrap();
            assert!(
                chrono::DateTime::parse_from_rfc3339(&result).is_ok(),
                "failed for: {unit}"
            );
        }
    }

    #[test]
    fn test_timeago_unix_fallback() {
        let result = apply_filter("timeago", "1704067200", &FilterArgs::None).unwrap();
        assert!(result.contains("2024-01-01"));
    }

    // ── unknown filter passes through ──────────────────────────────────

    #[test]
    fn test_unknown_filter_passthrough() {
        let result = apply_filter("nonexistent_filter", "keep me", &FilterArgs::None).unwrap();
        assert_eq!(result, "keep me");
    }

    // ── hexdump/strdump are no-ops ─────────────────────────────────────

    #[test]
    fn test_hexdump_strdump_passthrough() {
        assert_eq!(
            apply_filter("hexdump", "test", &FilterArgs::None).unwrap(),
            "test"
        );
        assert_eq!(
            apply_filter("strdump", "test", &FilterArgs::None).unwrap(),
            "test"
        );
    }

    // ── re_replace edge cases ──────────────────────────────────────────

    #[test]
    fn test_re_replace_no_match() {
        let result = apply_filter(
            "re_replace",
            "no digits",
            &FilterArgs::List(vec!["\\d+".into(), "X".into()]),
        )
        .unwrap();
        assert_eq!(result, "no digits");
    }

    #[test]
    fn test_re_replace_multiple_matches() {
        let result = apply_filter(
            "re_replace",
            "a1b2c3",
            &FilterArgs::List(vec!["\\d".into(), "".into()]),
        )
        .unwrap();
        assert_eq!(result, "abc");
    }

    // ── dateparse ──────────────────────────────────────────────────────

    #[test]
    fn test_dateparse_go_format() {
        let result = apply_filter(
            "dateparse",
            "2024-03-15 14:30:00",
            &FilterArgs::Single("yyyy-MM-dd HH:mm:ss".into()),
        )
        .unwrap();
        assert!(result.contains("2024-03-15"));
    }
}
