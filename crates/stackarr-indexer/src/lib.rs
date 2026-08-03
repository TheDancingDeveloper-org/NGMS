// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

pub mod indexarr;
pub mod manager;
pub mod newznab;
pub mod search;

pub use indexarr::{IndexarrClient, IndexarrStatus, RestSearchFilters};
pub use manager::IndexerManager;
pub use newznab::{NewznabClient, ReleaseInfo};
pub use search::{MovieSearchCriteria, SearchService, TextSearchCriteria, TvSearchCriteria};
