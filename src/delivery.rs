//! Delivering an approved report (D1–D5): a notice email that holds no
//! health information, only a signed link to the patient's page. The link
//! works for [`LINK_DAYS`] days.

use std::sync::Arc;

use uuid::Uuid;

use crate::engine::files::MasterKey;
use crate::engine::mail::Email;
use crate::engine::signed::Signer;

/// How long a patient's link works (D4).
pub const LINK_DAYS: i64 = 30;

/// Where patient links point and how they are signed.
#[derive(Clone)]
pub struct Delivery {
    base_url: String,
    signer: Arc<Signer>,
}

impl Delivery {
    /// Links under `base_url`, signed with a key derived from `master` for
    /// this purpose only.
    pub fn new(master: &MasterKey, base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            signer: Arc::new(Signer::new(master, "lab-report-link")),
        }
    }

    /// The patient's link for delivery `id`.
    pub fn link(&self, id: Uuid) -> String {
        format!("{}/r/{}", self.base_url, self.signer.token(id))
    }

    /// The delivery a link's token names, if its signature is right.
    pub fn delivery_of(&self, token: &str) -> Option<Uuid> {
        self.signer.verify(token)
    }

    /// The notice for delivery `id`: the clinic's name and the link, and
    /// nothing about the results (D3).
    pub fn notice(&self, clinic_name: &str, to_address: &str, id: Uuid) -> Email {
        Email {
            to_address: to_address.to_owned(),
            from_name: clinic_name.to_owned(),
            reply_to: None,
            subject: format!("Your lab report from {clinic_name} is ready"),
            body_text: format!(
                "Hello,\n\nYour lab report from {clinic_name} is ready, reviewed by your physician. Open it here:\n\n\
                 {}\n\nThe link works for {LINK_DAYS} days. If you have questions, contact your clinic.\n",
                self.link(id)
            ),
        }
    }
}
