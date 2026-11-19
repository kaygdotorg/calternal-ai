//! Commit and replay a fixture effect through the real receipt journal.
use super::{Authority, Duration, Uuid, json};
use std::path::Path;

pub(super) struct Effects {
    pub(super) epoch: Uuid,
    pub(super) ticket: String,
    pub(super) fenced: bool,
    pub(super) content: calternal_content::Content,
    pub(super) authority: Authority,
}
impl Effects {
    pub(super) fn apply(&mut self, epoch: Uuid, ticket: &str, key: &str) -> Option<bool> {
        if self.fenced || self.epoch != epoch || self.ticket != ticket {
            return None;
        }
        let Authority {
            account_id,
            space_id,
            ..
        } = self.authority;
        if let Some(receipt) = self.content.receipt(space_id, account_id, key).unwrap() {
            assert_eq!(receipt.fingerprint, "fixture-v1");
            assert_eq!(receipt.result, json!({"ok": true}));
            return Some(false);
        }
        self.content
            .record_receipt(
                space_id,
                calternal_content::Request {
                    account: account_id,
                    via: None,
                    id: key,
                    operation: "spike.effect",
                    fingerprint: "fixture-v1",
                },
                json!({"ok": true}),
            )
            .unwrap();
        Some(true)
    }
}

pub(super) fn effect_content(root: &Path) -> calternal_content::Content {
    calternal_content::Content::open(
        calternal_roots::Roots {
            appdata: root.join("appdata"),
            userdata_ssd: root.join("ssd"),
            userdata_hdd: root.join("hdd"),
        },
        root.join("secrets"),
        Duration::ZERO,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap()
}
