//! Contract tests for spend limits and increase requests, served from doc-derived fixtures (see
//! `fixtures/README.md`).

mod common;

use claude_admin::{Error, IncreaseRequestResolver, IncreaseRequestStatus, SpendLimitScope, SpendPeriod};
use common::{assert_page, assert_round_trip, client, fixture, no_requests, ok, serve};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer};

#[tokio::test]
async fn decodes_effective_spend_limits() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/spend_limits/effective", "spend_limits_effective_list").await;

    let page = client(&server).effective_spend_limits().send().await.unwrap().body;
    assert_page(&page.data, &fixture("spend_limits_effective_list"));
    let sources: Vec<_> = page.data.iter().map(|row| row.source.kind()).collect();
    assert_eq!(sources, ["user", "seat_tier", "rbac_group", "organization_service", "organization", "user"]);
    assert_eq!(page.data[4].source, SpendLimitScope::Organization);

    let deleted = &page.data[5];
    assert!(deleted.actor.deleted);
    assert_eq!((deleted.actor.email_address.as_ref(), deleted.amount.as_ref()), (None, None));
    assert_eq!(deleted.period, SpendPeriod::Daily);
}

#[tokio::test]
async fn decodes_spend_limit_and_increase_requests() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/spend_limits/spl_01ExampleSpendLimit0000", "spend_limits_retrieve").await;
    serve(&server, "/v1/organizations/spend_limit_increase_requests", "spend_limit_increase_requests_list").await;
    serve(
        &server,
        "/v1/organizations/spend_limit_increase_requests/spend_limit_increase_request_example",
        "spend_limit_increase_requests_retrieve",
    )
    .await;
    let client = client(&server);

    let limit = client.spend_limit("spl_01ExampleSpendLimit0000").await.unwrap().body;
    assert!(
        matches!(&limit.scope, SpendLimitScope::RbacGroup(g) if g.rbac_group_id == "rbac_group_01ExampleGroup00000")
    );
    assert_round_trip(&limit, &fixture("spend_limits_retrieve"));

    let requests = client.spend_limit_increase_requests().send().await.unwrap().body;
    assert_page(&requests.data, &fixture("spend_limit_increase_requests_list"));
    let [approved, denied, pending] = &requests.data[..] else { panic!("three requests") };
    assert!(
        matches!(&approved.resolved_by, Some(IncreaseRequestResolver::User(u)) if u.user_id == "user_01ExampleUser000000000")
    );
    assert!(matches!(&denied.resolved_by, Some(IncreaseRequestResolver::ScopedApiKey(_))));
    assert_eq!(pending.status, IncreaseRequestStatus::Pending);
    assert_eq!(pending.spend_summary.as_ref().map(|s| s.period_to_date_spend.as_str()), Some("12050.5"));

    let request = client.spend_limit_increase_request("spend_limit_increase_request_example").await.unwrap().body;
    assert_round_trip(&request, &fixture("spend_limit_increase_requests_retrieve"));
}

#[tokio::test]
async fn filters_use_literal_wire_names() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/spend_limits/effective"))
        .respond_with(ok(fixture("spend_limits_effective_list")))
        .mount(&server)
        .await;
    Mock::given(path("/v1/organizations/spend_limit_increase_requests"))
        .respond_with(ok(fixture("spend_limit_increase_requests_list")))
        .mount(&server)
        .await;
    let client = client(&server);

    client
        .effective_spend_limits()
        .limit(10)
        .period(SpendPeriod::Monthly)
        .period(SpendPeriod::Daily)
        .user_id("user_01ExampleUser000000000")
        .send()
        .await
        .unwrap();
    client
        .spend_limit_increase_requests()
        .actor_id("user_01ExampleUser000000000")
        .status(IncreaseRequestStatus::Pending)
        .status(IncreaseRequestStatus::Denied)
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some("limit=10&period[]=monthly&period[]=daily&user_ids[]=user_01ExampleUser000000000")
    );
    assert_eq!(
        requests[1].url.query(),
        Some("actor_ids[]=user_01ExampleUser000000000&status[]=pending&status[]=denied")
    );
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = no_requests().await;
    let client = client(&server);

    let four_periods = client
        .effective_spend_limits()
        .period(SpendPeriod::Daily)
        .period(SpendPeriod::Weekly)
        .period(SpendPeriod::Monthly)
        .period(SpendPeriod::Daily);
    assert!(matches!(four_periods.send().await, Err(Error::InvalidArgument(_))));

    let many_users = (0..101).fold(client.effective_spend_limits(), |request, n| request.user_id(format!("user_{n}")));
    assert!(matches!(many_users.send().await, Err(Error::InvalidArgument(_))));

    assert!(matches!(client.effective_spend_limits().limit(1001).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.spend_limit_increase_requests().limit(0).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.spend_limit(".").await, Err(Error::InvalidArgument(_))));
}
