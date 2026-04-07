use std::path::PathBuf;

/// Resolved paths to PostgreSQL binaries and directories.
#[derive(Debug, Clone)]
pub struct PgPaths {
    /// Directory containing pg_ctl, initdb, postgres, pg_isready, etc.
    pub bin_dir: PathBuf,
    /// PostgreSQL shared libraries directory.
    pub lib_dir: PathBuf,
    /// PostgreSQL share directory (timezone data, etc.).
    pub share_dir: PathBuf,
}

/// PostgreSQL database mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgMode {
    /// User provides their own PostgreSQL instance and connection string.
    External,
    /// StackArr downloads and manages a PostgreSQL instance.
    Managed,
    /// PostgreSQL binaries are embedded in the binary (requires `embed` feature).
    Embedded,
}

impl PgMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "external" => Some(Self::External),
            "managed" => Some(Self::Managed),
            "embedded" => Some(Self::Embedded),
            _ => None,
        }
    }
}

/// Version metadata written to `{data_dir}/postgres/version.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PgVersionInfo {
    pub pg_major: u32,
    pub pg_version: String,
    pub provisioned_at: chrono::DateTime<chrono::Utc>,
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pg_mode_from_str() {
        assert_eq!(PgMode::from_str("external"), Some(PgMode::External));
        assert_eq!(PgMode::from_str("managed"), Some(PgMode::Managed));
        assert_eq!(PgMode::from_str("embedded"), Some(PgMode::Embedded));
        assert_eq!(PgMode::from_str("invalid"), None);
    }

    #[test]
    fn test_pg_version_info_roundtrip() {
        let info = PgVersionInfo {
            pg_major: 17,
            pg_version: "17.4".to_string(),
            provisioned_at: chrono::Utc::now(),
            source: "managed".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: PgVersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pg_major, 17);
        assert_eq!(parsed.pg_version, "17.4");
        assert_eq!(parsed.source, "managed");
    }
}
