//! C1–C7 and E3: the physician's review of a draft.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::sync::Arc;

use common::{Fake, clock, draft_now, imported};
use lab_report_assistant::drafting::Explanation;
use lab_report_assistant::drafts::{Drafts, Source, content_hash};
use lab_report_assistant::engine::accounts::{Members, Role, Session};
use lab_report_assistant::engine::clock::FixedClock;
use lab_report_assistant::lab_reports::{ClinicId, LabReportId};
use lab_report_assistant::review::{ReviewError, Reviews};
use sqlx::PgPool;

struct Setup {
    pool: PgPool,
    clock: Arc<FixedClock>,
    clinic: ClinicId,
    report: LabReportId,
    physician: Session,
    admin: Session,
}

impl Setup {
    fn reviews(&self) -> Reviews {
        Reviews::new(self.pool.clone(), self.clock.clone())
    }

    async fn latest(&self) -> lab_report_assistant::drafts::StoredDraft {
        Drafts::new(self.pool.clone())
            .latest(self.clinic, self.report)
            .await
            .unwrap()
            .unwrap()
    }

    async fn approve_latest(&self, who: &Session) -> Result<(), ReviewError> {
        let draft = self.latest().await;
        self.reviews()
            .approve(who, self.report, draft.id, &content_hash(&draft.explanations))
            .await
    }
}

/// Report 12, drafted by the model with `model`, with a physician and the
/// clinic's administrator (its owner).
async fn setup(pool: PgPool, model: Fake) -> Setup {
    let clock = clock();
    let (clinic, report) = imported(&pool, clock.clone()).await;
    draft_now(&pool, clock.clone(), model).await;
    let reviews = Reviews::new(pool.clone(), clock.clone());
    let doctor = reviews
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let owner = Members::new(pool.clone(), clock.clone()).list(clinic).await.unwrap();
    let owner = owner.iter().find(|m| m.role == Role::Owner).unwrap();
    Setup {
        physician: Session {
            tenant: clinic,
            member: doctor,
            email: "dr.example@example.test".into(),
            role: Role::Staff,
        },
        admin: Session {
            tenant: clinic,
            member: owner.id,
            email: owner.email.clone(),
            role: Role::Owner,
        },
        pool,
        clock,
        clinic,
        report,
    }
}

fn explanations(count: usize) -> Vec<Explanation> {
    (1..=count)
        .map(|n| Explanation {
            result: format!("r{n}"),
            text: format!("Written by the physician for result {n}."),
        })
        .collect()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c1_the_physician_can_approve_a_passing_draft(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.approve_latest(&s.physician).await.unwrap();
    assert!(
        s.reviews()
            .current_approval(s.clinic, s.report)
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c1_the_administrator_cannot_approve(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    assert!(matches!(
        s.approve_latest(&s.admin).await,
        Err(ReviewError::NotAPhysician)
    ));
    assert!(
        s.reviews()
            .current_approval(s.clinic, s.report)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c2_a_blocked_draft_cannot_be_approved(pool: PgPool) {
    let s = setup(pool, Fake(|answer| Ok(answer.replace("one thing", "about 5 things")))).await;
    assert!(matches!(
        s.approve_latest(&s.physician).await,
        Err(ReviewError::Blocked)
    ));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c3_an_approval_must_name_the_exact_text_the_physician_saw(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    let draft = s.latest().await;
    let mut other = draft.explanations.clone();
    other[0].text.push_str(" Changed.");

    let refused = s
        .reviews()
        .approve(&s.physician, s.report, draft.id, &content_hash(&other))
        .await;

    assert!(matches!(refused, Err(ReviewError::Changed)));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c3_a_change_after_approval_needs_approving_again(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.approve_latest(&s.physician).await.unwrap();

    s.reviews().edit(&s.physician, s.report, explanations(8)).await.unwrap();

    assert!(
        s.reviews()
            .current_approval(s.clinic, s.report)
            .await
            .unwrap()
            .is_none(),
        "the old approval no longer counts"
    );
    s.approve_latest(&s.physician).await.unwrap();
    assert!(
        s.reviews()
            .current_approval(s.clinic, s.report)
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c4_an_edit_that_leaves_out_a_result_cannot_be_approved(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;

    let edited = s.reviews().edit(&s.physician, s.report, explanations(7)).await.unwrap();

    assert!(edited.blocked);
    assert!(matches!(
        s.approve_latest(&s.physician).await,
        Err(ReviewError::Blocked)
    ));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c5_diagnosis_wording_in_a_physicians_edit_warns_but_can_be_approved(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    let mut text = explanations(8);
    text[3].text = "Your sodium is a little high, likely from mild dehydration.".into();

    let edited = s.reviews().edit(&s.physician, s.report, text).await.unwrap();

    assert!(!edited.blocked);
    assert_eq!(edited.warnings.len(), 1, "{:?}", edited.warnings);
    assert!(edited.warnings[0].contains("dehydration"));
    s.approve_latest(&s.physician).await.unwrap();
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c6_a_third_new_draft_is_refused(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    for _ in 0..2 {
        s.reviews()
            .reject(&s.physician, s.report, "Too technical")
            .await
            .unwrap();
        draft_now(&s.pool, s.clock.clone(), Fake(Ok)).await;
    }

    let third = s.reviews().reject(&s.physician, s.report, "Still too technical").await;

    assert!(matches!(third, Err(ReviewError::NoMoreDrafts)));
    assert_eq!(s.latest().await.attempt, 3);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c7_a_report_written_by_hand_can_be_approved(pool: PgPool) {
    let s = setup(pool, Fake(|_| Ok("not json".into()))).await;

    s.reviews()
        .write_by_hand(&s.physician, s.report, explanations(8))
        .await
        .unwrap();

    assert_eq!(s.latest().await.source, Source::Physician);
    s.approve_latest(&s.physician).await.unwrap();
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e3_a_reports_timeline_lists_its_steps_in_order(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.reviews().edit(&s.physician, s.report, explanations(8)).await.unwrap();
    s.approve_latest(&s.physician).await.unwrap();

    let steps: Vec<(String, String)> = s
        .reviews()
        .timeline(s.clinic, s.report)
        .await
        .unwrap()
        .into_iter()
        .map(|entry| (entry.actor, entry.action))
        .collect();

    let expected = [
        ("system", "lab_report.imported"),
        ("system", "lab_report.drafted"),
        ("system", "lab_report.checks_passed"),
        ("dr.example@example.test", "lab_report.edited"),
        ("dr.example@example.test", "lab_report.approved"),
    ];
    assert_eq!(steps, expected.map(|(a, b)| (a.to_owned(), b.to_owned())));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e3_the_clinic_wide_log_filters_by_person_and_action(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.approve_latest(&s.physician).await.unwrap();

    let by_doctor = s
        .reviews()
        .clinic_log(s.clinic, Some("dr.example@example.test"), None)
        .await
        .unwrap();
    let approvals = s
        .reviews()
        .clinic_log(s.clinic, None, Some("lab_report.approved"))
        .await
        .unwrap();
    let all = s.reviews().clinic_log(s.clinic, None, None).await.unwrap();

    assert_eq!(by_doctor.len(), 1);
    assert_eq!(approvals.len(), 1);
    assert!(all.len() >= 4);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c4_an_edit_with_a_blank_explanation_cannot_be_approved(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    let mut text = explanations(8);
    text[2].text = "   ".into();

    let edited = s.reviews().edit(&s.physician, s.report, text).await.unwrap();

    assert!(edited.blocked);
    assert!(matches!(
        s.approve_latest(&s.physician).await,
        Err(ReviewError::Blocked)
    ));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c6_a_rejected_draft_cannot_be_approved_while_the_new_one_is_written(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;

    s.reviews()
        .reject(&s.physician, s.report, "Too technical")
        .await
        .unwrap();

    assert!(matches!(
        s.approve_latest(&s.physician).await,
        Err(ReviewError::Rejected)
    ));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c6_rejecting_again_before_the_new_draft_arrives_is_refused(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.reviews()
        .reject(&s.physician, s.report, "Too technical")
        .await
        .unwrap();

    let again = s.reviews().reject(&s.physician, s.report, "Still waiting").await;

    assert!(matches!(again, Err(ReviewError::Rejected)));
    let queued: i64 = sqlx::query_scalar("select count(*) from jobs where status = 'queued'")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(queued, 1, "one new draft asked for, not two");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c7_a_new_draft_never_replaces_text_the_physician_wrote_meanwhile(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;
    s.reviews()
        .reject(&s.physician, s.report, "Too technical")
        .await
        .unwrap();
    s.reviews()
        .write_by_hand(&s.physician, s.report, explanations(8))
        .await
        .unwrap();

    draft_now(&s.pool, s.clock.clone(), Fake(Ok)).await;

    assert_eq!(s.latest().await.source, Source::Physician);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e1_approving_the_same_text_twice_is_logged_once(pool: PgPool) {
    let s = setup(pool, Fake(Ok)).await;

    s.approve_latest(&s.physician).await.unwrap();
    s.approve_latest(&s.physician).await.unwrap();

    let approvals = s
        .reviews()
        .clinic_log(s.clinic, None, Some("lab_report.approved"))
        .await
        .unwrap();
    assert_eq!(approvals.len(), 1);
}
