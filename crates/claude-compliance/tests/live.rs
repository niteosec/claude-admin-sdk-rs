//! Opt-in checks against the real API. Ignored by default; they need a key and write
//! `compliance_api_accessed` records into the organization's Activity Feed (retained for six years).
//!
//! ```sh
//! ANTHROPIC_COMPLIANCE_KEY=sk-ant-... cargo test -p claude-compliance --test live -- --ignored
//! ```

use claude_compliance::{ComplianceClient, activity_types};
use futures::TryStreamExt;

fn client() -> ComplianceClient {
    let key = std::env::var("ANTHROPIC_COMPLIANCE_KEY").expect("set ANTHROPIC_COMPLIANCE_KEY");
    ComplianceClient::new(key).unwrap()
}

#[tokio::test]
#[ignore = "needs ANTHROPIC_COMPLIANCE_KEY"]
async fn reads_a_page_with_metadata() {
    let page = client().activities().limit(5).send().await.unwrap();
    assert!(page.meta.request_id.is_some());
    assert!(page.meta.organization_id.is_some());
    assert!(page.body.data.len() <= 5);
}

#[tokio::test]
#[ignore = "needs ANTHROPIC_COMPLIANCE_KEY"]
async fn every_record_decodes_across_pages() {
    let activities: Vec<_> = client().activities().limit(100).stream().try_collect().await.unwrap();
    assert!(!activities.is_empty());
}

#[tokio::test]
#[ignore = "needs ANTHROPIC_COMPLIANCE_KEY"]
async fn a_read_appears_in_the_feed_under_its_request_id() {
    let client = client();
    let request_id = client.activities().limit(1).send().await.unwrap().meta.request_id.unwrap();
    // Observed latency is about a second; the documented bound is one minute.
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let page = client.activities().activity_type(activity_types::COMPLIANCE_API_ACCESSED).limit(50).send().await;
        let found = page.unwrap().body.data.iter().any(|activity| {
            activity.compliance_api_access().and_then(|access| access.request_id).as_deref()
                == Some(request_id.as_str())
        });
        if found {
            return;
        }
    }
    panic!("request {request_id} did not appear in the feed within 60 s");
}
