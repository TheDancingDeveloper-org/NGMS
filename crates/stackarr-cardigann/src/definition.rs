//! Serde structs for Cardigann YAML definitions.
//!
//! These mirror Prowlarr's `CardigannDefinition.cs` types and are deserialized
//! from the YAML files served at `https://indexers.prowlarr.com/master/{version}`.

use indexmap::IndexMap;
use serde::Deserialize;

/// Root definition loaded from a single YAML file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardigannDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub privacy: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default, rename = "requestDelay")]
    pub request_delay: Option<f64>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub legacylinks: Option<Vec<String>>,
    #[serde(default)]
    pub followredirect: Option<bool>,
    #[serde(default, rename = "testlinktorrent")]
    pub test_link_torrent: Option<bool>,
    #[serde(default)]
    pub certificates: Option<Vec<String>>,
    #[serde(default)]
    pub settings: Option<Vec<SettingsField>>,
    pub caps: CapabilitiesBlock,
    #[serde(default)]
    pub login: Option<LoginBlock>,
    #[serde(default)]
    pub ratio: Option<RatioBlock>,
    pub search: SearchBlock,
    #[serde(default)]
    pub download: Option<DownloadBlock>,
}

/// A user-configurable setting (API key, username, sort order, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct SettingsField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    #[serde(default)]
    pub defaults: Option<Vec<String>>,
    #[serde(default)]
    pub options: Option<IndexMap<String, String>>,
}

/// Indexer capability declarations — categories and search modes.
#[derive(Debug, Clone, Deserialize)]
pub struct CapabilitiesBlock {
    #[serde(default)]
    pub categories: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub categorymappings: Option<Vec<CategoryMappingBlock>>,
    #[serde(default)]
    pub modes: Option<IndexMap<String, Vec<String>>>,
    #[serde(default)]
    pub allowrawsearch: Option<bool>,
}

/// Maps an indexer-specific category ID to a Newznab standard category.
#[derive(Debug, Clone, Deserialize)]
pub struct CategoryMappingBlock {
    pub id: serde_yaml::Value,
    pub cat: String,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub default: Option<bool>,
}

// ---------------------------------------------------------------------------
// Login
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct LoginBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub submitpath: Option<String>,
    #[serde(default)]
    pub cookies: Option<Vec<String>>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default)]
    pub selectors: Option<bool>,
    #[serde(default)]
    pub inputs: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub selectorinputs: Option<IndexMap<String, SelectorBlock>>,
    #[serde(default)]
    pub getselectorinputs: Option<IndexMap<String, SelectorBlock>>,
    #[serde(default)]
    pub error: Option<Vec<ErrorBlock>>,
    #[serde(default)]
    pub test: Option<PageTestBlock>,
    #[serde(default)]
    pub captcha: Option<CaptchaBlock>,
    #[serde(default)]
    pub headers: Option<IndexMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaptchaBlock {
    #[serde(rename = "type")]
    pub captcha_type: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub input: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageTestBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub message: Option<SelectorBlock>,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SearchBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub paths: Option<Vec<SearchPathBlock>>,
    #[serde(default)]
    pub headers: Option<IndexMap<String, Vec<String>>>,
    #[serde(default)]
    pub keywordsfilters: Option<Vec<FilterBlock>>,
    #[serde(default, rename = "allowEmptyInputs")]
    pub allow_empty_inputs: Option<bool>,
    #[serde(default)]
    pub inputs: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub error: Option<Vec<ErrorBlock>>,
    #[serde(default)]
    pub preprocessingfilters: Option<Vec<FilterBlock>>,
    pub rows: RowsBlock,
    pub fields: IndexMap<String, FieldBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchPathBlock {
    pub path: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub inputs: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub queryseparator: Option<String>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub inheritinputs: Option<bool>,
    #[serde(default)]
    pub followredirect: Option<bool>,
    #[serde(default)]
    pub response: Option<ResponseBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseBlock {
    #[serde(rename = "type")]
    pub response_type: Option<String>,
    #[serde(default, rename = "noResultsMessage")]
    pub no_results_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RowsBlock {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub remove: Option<String>,
    #[serde(default)]
    pub filters: Option<Vec<FilterBlock>>,
    #[serde(default, rename = "case")]
    pub case_map: Option<IndexMap<String, serde_yaml::Value>>,
    #[serde(default)]
    pub after: Option<usize>,
    #[serde(default)]
    pub dateheaders: Option<SelectorBlock>,
    #[serde(default)]
    pub count: Option<SelectorBlock>,
    #[serde(default)]
    pub multiple: Option<bool>,
    #[serde(default, rename = "missingAttributeEqualsNoResults")]
    pub missing_attribute_equals_no_results: Option<bool>,
}

// ---------------------------------------------------------------------------
// Field / Selector / Filter primitives
// ---------------------------------------------------------------------------

/// A single field extraction rule (used in `search.fields`).
/// This is a superset of `SelectorBlock` — it can have `selector`, `text`,
/// `attribute`, `filters`, `optional`, `default`, and `case`.
#[derive(Debug, Clone, Deserialize)]
pub struct FieldBlock {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub remove: Option<String>,
    #[serde(default)]
    pub filters: Option<Vec<FilterBlock>>,
    #[serde(default, rename = "case")]
    pub case_map: Option<IndexMap<String, serde_yaml::Value>>,
}

/// A CSS/JSON selector for extracting a single value from a document.
#[derive(Debug, Clone, Deserialize)]
pub struct SelectorBlock {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub remove: Option<String>,
    #[serde(default)]
    pub filters: Option<Vec<FilterBlock>>,
    #[serde(default, rename = "case")]
    pub case_map: Option<IndexMap<String, serde_yaml::Value>>,
}

/// A transform applied to an extracted value.
#[derive(Debug, Clone, Deserialize)]
pub struct FilterBlock {
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_filter_args")]
    pub args: FilterArgs,
}

/// Filter arguments — can be absent, a single string, or a list of strings.
#[derive(Debug, Clone, Default)]
pub enum FilterArgs {
    #[default]
    None,
    Single(String),
    List(Vec<String>),
}

fn deserialize_filter_args<'de, D>(deserializer: D) -> Result<FilterArgs, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct FilterArgsVisitor;

    impl<'de> de::Visitor<'de> for FilterArgsVisitor {
        type Value = FilterArgs;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string, list of strings, or null")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(FilterArgs::None)
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(FilterArgs::None)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v.to_owned()))
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v))
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v.to_string()))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v.to_string()))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v.to_string()))
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Ok(FilterArgs::Single(v.to_string()))
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while let Some(item) = seq.next_element::<serde_yaml::Value>()? {
                match item {
                    serde_yaml::Value::String(s) => items.push(s),
                    serde_yaml::Value::Number(n) => items.push(n.to_string()),
                    serde_yaml::Value::Bool(b) => items.push(b.to_string()),
                    other => items.push(format!("{other:?}")),
                }
            }
            Ok(FilterArgs::List(items))
        }
    }

    deserializer.deserialize_any(FilterArgsVisitor)
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadBlock {
    #[serde(default)]
    pub selectors: Option<Vec<SelectorField>>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub before: Option<BeforeBlock>,
    #[serde(default)]
    pub infohash: Option<InfohashBlock>,
    #[serde(default)]
    pub headers: Option<IndexMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectorField {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub usebeforeresponse: Option<bool>,
    #[serde(default)]
    pub filters: Option<Vec<FilterBlock>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfohashBlock {
    #[serde(default)]
    pub hash: Option<SelectorField>,
    #[serde(default)]
    pub title: Option<SelectorField>,
    #[serde(default)]
    pub usebeforeresponse: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeforeBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub inputs: Option<IndexMap<String, String>>,
    #[serde(default)]
    pub queryseparator: Option<String>,
    #[serde(default)]
    pub pathselector: Option<SelectorField>,
}

// ---------------------------------------------------------------------------
// Ratio
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RatioBlock {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub filters: Option<Vec<FilterBlock>>,
}

// ---------------------------------------------------------------------------
// Meta definition (from the index endpoint)
// ---------------------------------------------------------------------------

/// Lightweight metadata returned by the definition listing endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct CardigannMetaDefinition {
    pub id: String,
    pub file: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub privacy: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub implementation: Option<String>,
    #[serde(default)]
    pub links: Option<Vec<String>>,
    #[serde(default)]
    pub legacylinks: Option<Vec<String>>,
    #[serde(default)]
    pub settings: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub caps: Option<serde_json::Value>,
    #[serde(default)]
    pub sha: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yts_definition() {
        let yaml = include_str!("../../../test-fixtures/cardigann/yts.yml");
        let def: CardigannDefinition = serde_yaml::from_str(yaml).expect("failed to parse yts.yml");
        assert_eq!(def.id, "yts");
        assert_eq!(def.name, "YTS");
        assert!(def.search.fields.contains_key("title"));
        assert!(def.search.fields.contains_key("seeders"));
    }

    #[test]
    fn parse_thepiratebay_definition() {
        let yaml = include_str!("../../../test-fixtures/cardigann/thepiratebay.yml");
        let def: CardigannDefinition =
            serde_yaml::from_str(yaml).expect("failed to parse thepiratebay.yml");
        assert_eq!(def.id, "thepiratebay");
        assert!(def.settings.is_some());
        // TPB has 3 settings
        assert!(def.settings.as_ref().unwrap().len() >= 3);
    }

    #[test]
    fn parse_1337x_definition() {
        let yaml = include_str!("../../../test-fixtures/cardigann/1337x.yml");
        let def: CardigannDefinition =
            serde_yaml::from_str(yaml).expect("failed to parse 1337x.yml");
        assert_eq!(def.id, "1337x");
        // 1337x has a download block with selectors
        assert!(def.download.is_some());
        assert!(def.download.as_ref().unwrap().selectors.is_some());
    }
}
