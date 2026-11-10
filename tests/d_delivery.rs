//! D1 and D3: nothing reaches the outbox without an approval, and the notice
//! holds no test name, value or result.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{Fake, clock, draft_now, imported};
use lab_report_assistant::delivery::Delivery;
use lab_report_assistant::drafting::Explanation;
use lab_report_assistant::drafts::{Drafts, content_hash};
use lab_report_assistant::engine::accounts::{Role, Session};
use lab_report_assistant::review::Reviews;
use sqlx::PgPool;

/// A clinic with report 12 drafted, a physician, and reviews that deliver.
async fn setup(pool: &PgPool) -> (Reviews, Session, lab_report_assistant::lab_reports::LabReportId) {
    let clock = clock();
    let (clinic, report) = imported(pool, clock.clone()).await;
    draft_now(pool, clock.clone(), Fake(Ok)).await;
    let reviews = Reviews::new(pool.clone(), clock).with_delivery(Delivery::new(&common::master(), common::BASE_URL));
    let member = reviews
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let physician = Session {
        tenant: clinic,
        member,
        email: "dr.example@example.test".into(),
        role: Role::Staff,
    };
    (reviews, physician, report)
}

async fn notices(pool: &PgPool) -> Vec<(String, String, String, String)> {
    sqlx::query_as("select status, to_address, subject, body_text from outbox_messages order by created_at")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn approve(
    pool: &PgPool,
    reviews: &Reviews,
    who: &Session,
    report: lab_report_assistant::lab_reports::LabReportId,
) {
    let draft = Drafts::new(pool.clone())
        .latest(who.tenant, report)
        .await
        .unwrap()
        .unwrap();
    reviews
        .approve(who, report, draft.id, &content_hash(&draft.explanations))
        .await
        .unwrap();
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d1_nothing_reaches_the_outbox_before_the_physician_approves(pool: PgPool) {
    let (reviews, physician, report) = setup(&pool).await;
    let text: Vec<Explanation> = (1..=8)
        .map(|n| Explanation {
            result: format!("r{n}"),
            text: format!("Result {n}."),
        })
        .collect();
    reviews.edit(&physician, report, text).await.unwrap();
    reviews.reject(&physician, report, "Too short").await.unwrap();
    assert!(notices(&pool).await.is_empty());

    draft_now(&pool, clock(), Fake(Ok)).await;
    approve(&pool, &reviews, &physician, report).await;

    let sent = notices(&pool).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].0.as_str(), sent[0].1.as_str()),
        ("approved", "lena.estrada@example.test")
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d3_the_notice_holds_no_test_name_value_or_result(pool: PgPool) {
    let (reviews, physician, report) = setup(&pool).await;
    approve(&pool, &reviews, &physician, report).await;

    let (_, _, subject, body) = notices(&pool).await.remove(0);

    assert_eq!(subject, "Your lab report from Example Womens Health is ready");
    assert!(body.contains(&format!("{}/r/", common::BASE_URL)), "{body}");
    // The signed link's hex could hold any digits; the rest of the notice can't.
    let body: String = body
        .split_whitespace()
        .filter(|word| !word.contains("/r/"))
        .collect::<Vec<_>>()
        .join(" ");
    let bundle =
        lab_report_assistant::fhir::read_lab_report(&lab_report_assistant::demo_data::generate()[11].json).unwrap();
    for result in &bundle.results {
        for leak in [result.name.as_str(), &result.value.to_string(), &result.code] {
            assert!(
                !subject.contains(leak) && !body.contains(leak),
                "{leak} is in the notice"
            );
        }
    }
    for word in [
        "above",
        "below",
        "within",
        "range",
        "high",
        "low",
        "Basic metabolic panel",
    ] {
        assert!(
            !body.to_lowercase().contains(&word.to_lowercase()),
            "{word} is in the notice"
        );
    }
}
