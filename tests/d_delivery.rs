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

/// A mail provider that only counts what it was asked to send.
#[derive(Clone, Default)]
struct Counting(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl lab_report_assistant::engine::mail::Mailer for Counting {
    fn blocks_duplicate_keys(&self) -> bool {
        false
    }

    async fn send(
        &self,
        email: &lab_report_assistant::engine::mail::Email,
        _key: &str,
    ) -> Result<lab_report_assistant::engine::mail::SendReceipt, lab_report_assistant::engine::mail::MailError> {
        self.0.lock().unwrap().push(email.to_address.clone());
        Ok(lab_report_assistant::engine::mail::SendReceipt {
            provider_message_id: "m1".into(),
        })
    }
}

async fn send_now(pool: &PgPool, mailer: &Counting) {
    use lab_report_assistant::engine::outbox::Sender;
    let checks = std::sync::Arc::new(lab_report_assistant::delivery::CurrentApproval::new(pool.clone()));
    Sender::new(pool.clone(), mailer.clone(), clock(), checks)
        .drain_once(10)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d1_an_approved_notice_is_sent(pool: PgPool) {
    let (reviews, physician, report) = setup(&pool).await;
    approve(&pool, &reviews, &physician, report).await;
    let mailer = Counting::default();

    send_now(&pool, &mailer).await;

    assert_eq!(*mailer.0.lock().unwrap(), ["lena.estrada@example.test"]);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c3_a_notice_whose_report_changed_after_approval_is_not_sent(pool: PgPool) {
    let (reviews, physician, report) = setup(&pool).await;
    approve(&pool, &reviews, &physician, report).await;
    let text: Vec<Explanation> = (1..=8)
        .map(|n| Explanation {
            result: format!("r{n}"),
            text: format!("Result {n}."),
        })
        .collect();
    reviews.edit(&physician, report, text).await.unwrap();
    let mailer = Counting::default();

    send_now(&pool, &mailer).await;

    assert!(mailer.0.lock().unwrap().is_empty());
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d1_a_notice_left_unapproved_by_an_interruption_is_approved_by_the_sweep(pool: PgPool) {
    let (reviews, physician, report) = setup(&pool).await;
    approve(&pool, &reviews, &physician, report).await;
    // As if the process stopped between the approval and the engine's approval.
    sqlx::query(
        "update outbox_messages set status = 'draft', approved_hash = null, approved_at = null, approved_by = null",
    )
    .execute(&pool)
    .await
    .unwrap();

    lab_report_assistant::delivery::approve_waiting_notices(&pool, clock())
        .await
        .unwrap();
    let mailer = Counting::default();
    send_now(&pool, &mailer).await;

    assert_eq!(mailer.0.lock().unwrap().len(), 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d1_a_message_that_isnt_a_notice_such_as_a_login_link_still_goes_out(pool: PgPool) {
    use lab_report_assistant::engine::outbox::{NoExtraChecks, Outbox};
    let (_, physician, _) = setup(&pool).await;
    let outbox = Outbox::new(pool.clone(), clock());
    let email = lab_report_assistant::engine::mail::Email {
        to_address: "dr.example@example.test".into(),
        from_name: "Example Womens Health".into(),
        reply_to: None,
        subject: "Your login link".into(),
        body_text: "Open http://lab.example.test/login/abc to log in.".into(),
    };
    let id = outbox.create_draft(physician.tenant, &email).await.unwrap();
    let message = outbox.get(physician.tenant, id).await.unwrap();
    outbox
        .approve(physician.tenant, id, "system", &message.content_hash(), &NoExtraChecks)
        .await
        .unwrap();
    let mailer = Counting::default();

    send_now(&pool, &mailer).await;

    assert_eq!(mailer.0.lock().unwrap().len(), 1);
}
