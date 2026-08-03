// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

//! Parity report generation.

use std::path::Path;

use anyhow::Result;

use crate::comparator::ParityResult;

/// Write a markdown parity report.
pub fn write_report(results: &[ParityResult], path: &Path) -> Result<()> {
    let mut md = String::new();

    md.push_str(&format!(
        "# Cardigann Parity Report — {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));

    // Summary
    let total = results.len();
    let perfect = results.iter().filter(|r| r.parity_pct >= 99.0).count();
    let good = results
        .iter()
        .filter(|r| r.parity_pct >= 80.0 && r.parity_pct < 99.0)
        .count();
    let poor = results.iter().filter(|r| r.parity_pct < 80.0).count();
    let errored = results
        .iter()
        .filter(|r| r.prowlarr_error.is_some() || r.stackarr_error.is_some())
        .count();

    let avg_parity = if total > 0 {
        results.iter().map(|r| r.parity_pct).sum::<f64>() / total as f64
    } else {
        0.0
    };

    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **Tests run**: {total}\n"));
    md.push_str(&format!("- **Average parity**: {avg_parity:.1}%\n"));
    md.push_str(&format!(
        "- **Perfect match** (≥99%): {perfect} ({:.0}%)\n",
        if total > 0 {
            perfect as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    ));
    md.push_str(&format!("- **Good** (80-99%): {good}\n"));
    md.push_str(&format!("- **Poor** (<80%): {poor}\n"));
    md.push_str(&format!("- **Errors**: {errored}\n\n"));

    // Per-indexer table
    md.push_str("## Per-Indexer Results\n\n");
    md.push_str("| Indexer | Query | Prowlarr | StackArr | Match | Parity | Issues |\n");
    md.push_str("|---------|-------|----------|----------|-------|--------|--------|\n");

    for r in results {
        let issues = if let Some(ref e) = r.prowlarr_error {
            format!("Prowlarr: {}", truncate(e, 40))
        } else if let Some(ref e) = r.stackarr_error {
            format!("StackArr: {}", truncate(e, 40))
        } else if !r.size_mismatches.is_empty() {
            format!("{} size mismatches", r.size_mismatches.len())
        } else if r.parity_pct >= 99.0 {
            "-".to_owned()
        } else {
            format!("+{} / -{}", r.stackarr_only.len(), r.prowlarr_only.len())
        };

        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.0}% | {} |\n",
            r.indexer_name,
            r.query,
            r.prowlarr_count,
            r.stackarr_count,
            r.matched_titles,
            r.parity_pct,
            issues,
        ));
    }

    // Detailed mismatches for poor results
    let poor_results: Vec<_> = results.iter().filter(|r| r.parity_pct < 80.0).collect();
    if !poor_results.is_empty() {
        md.push_str("\n## Detailed Mismatches\n\n");
        for r in &poor_results {
            md.push_str(&format!(
                "### {} — \"{}\" ({:.0}%)\n\n",
                r.indexer_name, r.query, r.parity_pct
            ));

            if !r.prowlarr_only.is_empty() {
                md.push_str("**Prowlarr only:**\n");
                for t in r.prowlarr_only.iter().take(10) {
                    md.push_str(&format!("- {t}\n"));
                }
            }
            if !r.stackarr_only.is_empty() {
                md.push_str("\n**StackArr only:**\n");
                for t in r.stackarr_only.iter().take(10) {
                    md.push_str(&format!("- {t}\n"));
                }
            }
            md.push('\n');
        }
    }

    std::fs::write(path, &md)?;
    Ok(())
}

/// Print a summary to stdout.
pub fn print_summary(results: &[ParityResult]) {
    let total = results.len();
    if total == 0 {
        println!("No results to summarize.");
        return;
    }

    let perfect = results.iter().filter(|r| r.parity_pct >= 99.0).count();
    let avg = results.iter().map(|r| r.parity_pct).sum::<f64>() / total as f64;
    let errored = results
        .iter()
        .filter(|r| r.prowlarr_error.is_some() || r.stackarr_error.is_some())
        .count();

    println!("\n--- Parity Summary ---");
    println!("Tests:    {total}");
    println!("Average:  {avg:.1}%");
    println!("Perfect:  {perfect}/{total}");
    println!("Errors:   {errored}/{total}");

    // Show worst performers
    let mut sorted: Vec<_> = results.iter().collect();
    sorted.sort_by(|a, b| a.parity_pct.partial_cmp(&b.parity_pct).unwrap());

    println!("\nWorst 5:");
    for r in sorted.iter().take(5) {
        println!(
            "  {:<30} \"{:<10}\"  {:.0}%  (P:{} S:{})",
            r.indexer_name, r.query, r.parity_pct, r.prowlarr_count, r.stackarr_count
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}...", &s[..max])
    }
}
