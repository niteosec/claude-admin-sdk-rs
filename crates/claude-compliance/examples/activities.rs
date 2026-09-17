//! Prints the newest activities, skipping the feed's record of this program's own reads.
//!
//! ```sh
//! ANTHROPIC_COMPLIANCE_KEY=sk-ant-... cargo run -p claude-compliance --example activities
//! ```

use claude_compliance::{ComplianceClient, activity_types};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("ANTHROPIC_COMPLIANCE_KEY").map_err(|_| "set ANTHROPIC_COMPLIANCE_KEY")?;
    let client = ComplianceClient::new(key)?;
    println!("key type: {:?}", client.api_client().key_kind());

    let page = client.activities().limit(50).send().await?;
    println!(
        "organization {:?}, request {:?}, budget {:?}",
        page.meta.organization_id, page.meta.request_id, page.meta.rate_limit
    );
    for activity in page.body.data {
        if activity.activity_type == activity_types::COMPLIANCE_API_ACCESSED {
            continue;
        }
        println!("{}  {:<45} {}", activity.created_at, activity.activity_type, activity.actor.actor_type());
    }
    Ok(())
}
