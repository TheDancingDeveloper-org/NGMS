//! Category mapping between Cardigann indexer categories and Newznab standard categories.

use std::collections::HashMap;

use crate::definition::CapabilitiesBlock;

/// A resolved category mapping: indexer category ID → Newznab category IDs.
#[derive(Debug, Clone, Default)]
pub struct CategoryMapper {
    /// Indexer category string → list of Newznab category IDs.
    mappings: HashMap<String, Vec<i32>>,
    /// Reverse: Newznab category ID → list of indexer category strings.
    reverse: HashMap<i32, Vec<String>>,
}

impl CategoryMapper {
    /// Build a category mapper from a Cardigann capabilities block.
    pub fn from_caps(caps: &CapabilitiesBlock) -> Self {
        let mut mapper = Self::default();

        if let Some(ref mappings) = caps.categorymappings {
            for mapping in mappings {
                let indexer_id = match &mapping.id {
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };

                let newznab_id = newznab_name_to_id(&mapping.cat);

                mapper
                    .mappings
                    .entry(indexer_id.clone())
                    .or_default()
                    .push(newznab_id);

                mapper
                    .reverse
                    .entry(newznab_id)
                    .or_default()
                    .push(indexer_id);
            }
        }

        // Simple string-based categories (e.g., `categories: { "Movies": "Movies" }`)
        if let Some(ref cats) = caps.categories {
            for (key, cat_name) in cats {
                let newznab_id = newznab_name_to_id(cat_name);
                mapper
                    .mappings
                    .entry(key.clone())
                    .or_default()
                    .push(newznab_id);
                mapper
                    .reverse
                    .entry(newznab_id)
                    .or_default()
                    .push(key.clone());
            }
        }

        mapper
    }

    /// Map an indexer-specific category to Newznab category IDs.
    pub fn to_newznab(&self, indexer_cat: &str) -> Vec<i32> {
        self.mappings.get(indexer_cat).cloned().unwrap_or_default()
    }

    /// Map Newznab category IDs to indexer-specific category strings.
    pub fn from_newznab(&self, newznab_ids: &[i32]) -> Vec<String> {
        let mut result = Vec::new();
        for id in newznab_ids {
            if let Some(cats) = self.reverse.get(id) {
                result.extend(cats.clone());
            }
        }
        result.sort();
        result.dedup();
        result
    }
}

/// Map a Newznab category name (e.g., "Movies/HD") to its numeric ID.
///
/// Based on the Newznab standard category numbering scheme.
pub fn newznab_name_to_id(name: &str) -> i32 {
    match name {
        // Console
        "Console" => 1000,
        "Console/NDS" => 1010,
        "Console/PSP" => 1020,
        "Console/Wii" => 1030,
        "Console/XBox" => 1040,
        "Console/XBox 360" => 1050,
        "Console/Wiiware" => 1060,
        "Console/XBox 360 DLC" => 1070,
        "Console/PS3" => 1080,
        "Console/Other" => 1090,
        "Console/3DS" => 1110,
        "Console/PS Vita" => 1120,
        "Console/WiiU" => 1130,
        "Console/XBox One" => 1140,
        "Console/PS4" => 1180,

        // Movies
        "Movies" => 2000,
        "Movies/Foreign" => 2010,
        "Movies/Other" => 2020,
        "Movies/SD" => 2030,
        "Movies/HD" => 2040,
        "Movies/UHD" => 2045,
        "Movies/BluRay" => 2050,
        "Movies/3D" => 2060,
        "Movies/DVD" => 2070,
        "Movies/WEB-DL" => 2080,

        // Audio
        "Audio" => 3000,
        "Audio/MP3" => 3010,
        "Audio/Video" => 3020,
        "Audio/Audiobook" => 3030,
        "Audio/Lossless" => 3040,
        "Audio/Other" => 3050,
        "Audio/Foreign" => 3060,

        // PC
        "PC" => 4000,
        "PC/0day" => 4010,
        "PC/ISO" => 4020,
        "PC/Mac" => 4030,
        "PC/Mobile-Other" => 4040,
        "PC/Games" => 4050,
        "PC/Mobile-iOS" => 4060,
        "PC/Mobile-Android" => 4070,

        // TV
        "TV" => 5000,
        "TV/WEB-DL" => 5010,
        "TV/Foreign" => 5020,
        "TV/SD" => 5030,
        "TV/HD" => 5040,
        "TV/UHD" => 5045,
        "TV/Other" => 5050,
        "TV/Sport" => 5060,
        "TV/Anime" => 5070,
        "TV/Documentary" => 5080,

        // XXX
        "XXX" => 6000,
        "XXX/DVD" => 6010,
        "XXX/WMV" => 6020,
        "XXX/XviD" => 6030,
        "XXX/x264" => 6040,
        "XXX/UHD" => 6045,
        "XXX/Pack" => 6050,
        "XXX/ImageSet" => 6060,
        "XXX/Other" => 6070,
        "XXX/SD" => 6080,
        "XXX/WEB-DL" => 6090,

        // Books
        "Books" => 7000,
        "Books/Mags" => 7010,
        "Books/EBook" => 7020,
        "Books/Comics" => 7030,
        "Books/Technical" => 7040,
        "Books/Other" => 7050,
        "Books/Foreign" => 7060,

        // Other
        "Other" => 8000,
        "Other/Misc" => 8010,
        "Other/Hashed" => 8020,

        _ => 8010, // Default to Other/Misc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_mapping() {
        let caps = CapabilitiesBlock {
            categorymappings: Some(vec![
                CategoryMappingBlock {
                    id: serde_yaml::Value::Number(45.into()),
                    cat: "Movies/HD".into(),
                    desc: Some("720p".into()),
                    default: None,
                },
                CategoryMappingBlock {
                    id: serde_yaml::Value::Number(44.into()),
                    cat: "Movies/HD".into(),
                    desc: Some("1080p".into()),
                    default: None,
                },
            ]),
            categories: None,
            modes: None,
            allowrawsearch: None,
        };

        let mapper = CategoryMapper::from_caps(&caps);
        assert_eq!(mapper.to_newznab("45"), vec![2040]);
        assert_eq!(mapper.to_newznab("44"), vec![2040]);

        let indexer_cats = mapper.from_newznab(&[2040]);
        assert!(indexer_cats.contains(&"44".to_owned()));
        assert!(indexer_cats.contains(&"45".to_owned()));
    }
}
