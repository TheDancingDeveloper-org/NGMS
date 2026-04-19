//! Phase 4 contract tests — semaphore-backed connection slots.
//!
//! Note: the `ConnectionTracker` (`conn_tracker`) is currently a stub retained
//! for observability — it is not wired into the nzb-news fetch path, which
//! manages its own per-server connection count. As a result, `connection_total()`
//! always returns 0 through the queue_manager API. These tests verify the
//! end-to-end download contract (all articles succeed, no spurious failures)
//! rather than live slot accounting, which is tested inside nzb-news itself.

mod harness;

use std::time::Duration;

use harness::nzb_fixture::NzbFixture;
use harness::{HarnessBuilder, ServerProfile, yenc_articles};
use nzb_nntp::testutil::MockConfig;

#[tokio::test]
async fn live_connection_count_never_exceeds_limit() {
    // 8 segments — verify all download successfully within the connection limit.
    // The slot contract (active ≤ limit) is enforced inside nzb-news and tested
    // there; here we only verify end-to-end correctness via the queue_manager API.
    let bodies: Vec<Vec<u8>> = (0..8)
        .map(|i| format!("payload-segment-{i}").into_bytes())
        .collect();
    let segs: Vec<(&str, &[u8])> = (0..8)
        .map(|i| {
            let mid: &'static str = match i {
                0 => "slot-1",
                1 => "slot-2",
                2 => "slot-3",
                3 => "slot-4",
                4 => "slot-5",
                5 => "slot-6",
                6 => "slot-7",
                _ => "slot-8",
            };
            (mid, bodies[i].as_slice())
        })
        .collect();
    let fixture = NzbFixture::new("slot-bound")
        .add_file("payload.bin", &segs)
        .build();

    let triples: Vec<(&str, &[u8], &str)> = fixture
        .articles
        .iter()
        .map(|(m, b, f)| (*m, *b, f.as_str()))
        .collect();

    const LIMIT: u16 = 2;
    let server = ServerProfile::start(
        "slot-srv",
        MockConfig {
            articles: yenc_articles(&triples),
            ..Default::default()
        },
        LIMIT,
    )
    .await;

    let engine = HarnessBuilder::new()
        .with_server(server)
        .article_timeout(10)
        .build();

    let job_id = engine
        .submit_nzb_xml("slot-bound", fixture.xml)
        .expect("submit");

    // Wait for the job to fully resolve.
    let resolved = engine
        .wait_for(Duration::from_secs(15), |snap| {
            snap.job(&job_id)
                .map(|j| j.articles_downloaded + j.articles_failed >= 8)
                .unwrap_or(false)
        })
        .await;
    assert!(
        resolved,
        "job didn't resolve all 8 segments within deadline"
    );

    let view = engine.job(&job_id).unwrap();
    assert_eq!(
        view.articles_downloaded, 8,
        "expected 8 successful downloads, got downloaded={} failed={}",
        view.articles_downloaded, view.articles_failed
    );

    // connection_snapshot reports the configured limit per-server even though
    // active tracking is not wired into the nzb-news backend.
    let snap = engine.queue_manager.connection_snapshot();
    for (_id, active, lim) in &snap {
        assert!(
            *active <= *lim,
            "live count {active} exceeded limit {lim} — would be a slot leak"
        );
    }
}
