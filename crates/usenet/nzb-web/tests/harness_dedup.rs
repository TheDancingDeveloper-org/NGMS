//! Integration test for content-hash dedup in `QueueManager::add_job`.
//!
//! Background: two submissions that share the same set of NNTP message-ids
//! would previously both enter the queue, both get promoted to Downloading,
//! and their WorkItems would interleave in the shared dispatcher queue.
//! Second-submitted duplicate jobs would appear stuck at 0/0/0 for minutes
//! while their twin drained identical articles.
//!
//! Fix: at `add_job` time, hash the sorted set of article message-ids.
//! Reject the second submission if the hash matches an active/queued job.

mod harness;

use harness::nzb_fixture::NzbFixture;
use harness::{HarnessBuilder, ServerProfile, yenc_articles};
use nzb_nntp::testutil::MockConfig;

#[tokio::test]
async fn second_submission_with_identical_message_ids_is_rejected() {
    // Same message-ids, same body → same content hash.
    let body_a = b"duplicate test body A";
    let body_b = b"duplicate test body B";
    let fix_a = NzbFixture::new("first-upload")
        .add_file(
            "payload.bin",
            &[
                ("dup-msg-1@test", body_a.as_slice()),
                ("dup-msg-2@test", body_b.as_slice()),
            ],
        )
        .build();
    // Build a second NZB whose XML differs (different name) but whose
    // message-ids are identical to the first. This is the prod scenario:
    // the same content posted twice, or a scheduler task double-submitting.
    let fix_b = NzbFixture::new("second-upload")
        .add_file(
            "payload.bin",
            &[
                ("dup-msg-1@test", body_a.as_slice()),
                ("dup-msg-2@test", body_b.as_slice()),
            ],
        )
        .build();

    let triples: Vec<(&str, &[u8], &str)> = fix_a
        .articles
        .iter()
        .map(|(m, b, f)| (*m, *b, f.as_str()))
        .collect();
    let articles = yenc_articles(&triples);

    let server = ServerProfile::start(
        "dedup-srv",
        MockConfig {
            articles,
            ..Default::default()
        },
        2,
    )
    .await;
    let engine = HarnessBuilder::new()
        .with_server(server)
        .article_timeout(10)
        .build();

    // First submission succeeds.
    let first_id = engine
        .submit_nzb_xml("first-upload", fix_a.xml)
        .expect("first submission should succeed");
    assert!(!first_id.is_empty());

    // Second submission must be rejected.
    let err = engine
        .submit_nzb_xml("second-upload", fix_b.xml)
        .expect_err("second submission with identical message-ids must be rejected");

    // Error message should identify the conflict.
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate"),
        "error should mention duplicate content, got: {msg}"
    );
    assert!(
        msg.contains(&first_id),
        "error should reference the existing job id ({first_id}), got: {msg}"
    );
}

#[tokio::test]
async fn disjoint_message_ids_are_not_rejected() {
    // Two NZBs with distinct message-ids — both should be accepted.
    let body = b"x";
    let fix_a = NzbFixture::new("nzb-a")
        .add_file("a.bin", &[("a-msg-1@test", body.as_slice())])
        .build();
    let fix_b = NzbFixture::new("nzb-b")
        .add_file("b.bin", &[("b-msg-1@test", body.as_slice())])
        .build();

    let mut triples: Vec<(&str, &[u8], &str)> = fix_a
        .articles
        .iter()
        .map(|(m, b, f)| (*m, *b, f.as_str()))
        .collect();
    triples.extend(fix_b.articles.iter().map(|(m, b, f)| (*m, *b, f.as_str())));
    let articles = yenc_articles(&triples);

    let server = ServerProfile::start(
        "disjoint-srv",
        MockConfig {
            articles,
            ..Default::default()
        },
        2,
    )
    .await;
    let engine = HarnessBuilder::new()
        .with_server(server)
        .article_timeout(10)
        .build();

    let a = engine.submit_nzb_xml("nzb-a", fix_a.xml).expect("a ok");
    let b = engine.submit_nzb_xml("nzb-b", fix_b.xml).expect("b ok");
    assert_ne!(a, b);
}
