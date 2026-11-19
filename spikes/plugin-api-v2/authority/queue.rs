//! Test the same space authority fence on the real job and approval engines.
use super::*;
use calternal_core::{AgentMode, OperationRisk};
use calternal_server::{
    plugins::{
        Session,
        jobs::{Admission, Job},
    },
    turns::access::Authority,
};

impl Host {
    pub(crate) async fn queue_races(&self, repeats: usize) {
        let actor = self.viewer;
        let who = fixture::identity(actor);
        let view = self.views[&self.shared].clone();
        self.member_role(Some("editor")).await;
        for _ in 0..repeats {
            for stage in [4, 5, 6, 7] {
                let thread = Uuid::new_v4();
                let request = Uuid::new_v4().to_string();
                let (job, person_request) = {
                    let state = self.state.clone();
                    let space = view.space;
                    tokio::task::spawn_blocking(move || {
                        let space = space.to_string();
                        let jobs = &state.jobs;
                        let id = jobs
                            .admit(Admission {
                                space: space.clone(),
                                plugin: "calternal-orchestrator".into(),
                                account: actor,
                                session: Session::Static,
                                operation: "agent.turn".into(),
                                input: json!({"request":{"thread_id":thread}}),
                                parent: None,
                                generation: 0,
                                request_key: request,
                            })
                            .unwrap();
                        if stage == 4 {
                            return (jobs.get(&space, &id).unwrap().unwrap(), None);
                        }
                        let claim = jobs.claim(&space, &id).unwrap().unwrap();
                        let job = claim.job;
                        let authority = Authority {
                            actor: calternal_types::Actor::agent(actor, thread),
                            space: space.clone(),
                            job: id.clone(),
                            mode: AgentMode::LeastAccess,
                            source_app: "notes".into(),
                            grants: vec![],
                        };
                        jobs.agent_prepare_scope(&authority, job.runs).unwrap();
                        jobs.agent_token(&authority, job.runs).unwrap();
                        jobs.checkpoint(
                            &space,
                            &id,
                            job.runs,
                            &jobs.live(&id),
                            &json!({"machine_id":Uuid::new_v4()}),
                        )
                        .unwrap();
                        if stage == 5 {
                            return (job, None);
                        }
                        let input = serde_json::from_value(
                            json!({"space_id":space,"title":"Queue fixture","markdown":"fixture"}),
                        )
                        .unwrap();
                        let approval = jobs
                            .agent_park_request(&job, || {
                                jobs.agent_action_request(
                                    &authority,
                                    "notes.create",
                                    &input,
                                    "authority-fixture",
                                    OperationRisk::Reversible,
                                    true,
                                )
                            })
                            .unwrap();
                        (job, Some(approval))
                    })
                    .await
                    .unwrap()
                };
                let status_before = self.queue_status(&job, person_request.as_deref()).await;
                assert_eq!(status_before.0, if stage == 4 { 0 } else { 1 });
                if stage >= 6 {
                    assert_eq!(status_before.2.as_deref(), Some("pending"));
                }
                if stage == 7 {
                    assert!(
                        self.queued_effect(&view, who, &job, person_request.as_deref(), stage)
                            .await
                    );
                    assert!(
                        self.queued_effect(&view, who, &job, person_request.as_deref(), stage)
                            .await
                    );
                    assert_eq!(
                        self.queue_status(&job, person_request.as_deref())
                            .await
                            .2
                            .as_deref(),
                        Some("once")
                    );
                } else {
                    self.arm(stage);
                    let (won, ()) = tokio::join!(
                        self.queued_effect(&view, who, &job, person_request.as_deref(), stage),
                        async {
                            self.wait_for_race().await;
                            self.member_role(None).await;
                            self.finish_race();
                        }
                    );
                    assert!(!won);
                    let status_after = self.queue_status(&job, person_request.as_deref()).await;
                    assert_eq!(status_after.0, status_before.0);
                    if stage == 5 {
                        assert!(status_after.1.is_null(), "a revoked job parked");
                    }
                    assert_ne!(status_after.2.as_deref(), Some("once"));
                    self.member_role(Some("editor")).await;
                }
                let state = self.state.clone();
                tokio::task::spawn_blocking(move || state.jobs.cancel(&job.space, &job.id))
                    .await
                    .unwrap()
                    .unwrap();
            }
        }
        eprintln!(
            "native queue races={} revoked_claims=0 revoked_parks=0 revoked_approvals=0 protected_approvals={repeats}",
            repeats * 3
        );
    }

    async fn queued_effect(
        &self,
        view: &View,
        who: Identity,
        job: &Job,
        approval: Option<&str>,
        stage: usize,
    ) -> bool {
        self.hooks.hit(stage).await;
        let _fence = view.gate.lock().await;
        if !self.editor(who, view).await {
            return false;
        }
        let state = self.state.clone();
        let job = job.clone();
        let approval = approval.map(str::to_owned);
        tokio::task::spawn_blocking(move || match stage {
            4 => state
                .jobs
                .claim(&job.space, &job.id)
                .is_ok_and(|job| job.is_some()),
            5 => state
                .jobs
                .agent_park_request(&job, || Ok("fixture-request".into()))
                .is_ok(),
            6 | 7 => state
                .jobs
                .agent_decide(
                    who.id,
                    approval.as_deref().unwrap(),
                    "queue-fixture-decision",
                    "once",
                    None,
                    |_| Ok(()),
                )
                .is_ok(),
            _ => unreachable!(),
        })
        .await
        .unwrap()
    }

    async fn queue_status(
        &self,
        job: &Job,
        approval: Option<&str>,
    ) -> (u32, Value, Option<String>) {
        let state = self.state.clone();
        let job = job.clone();
        let approval = approval.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            use rusqlite::OptionalExtension;
            let current = state.jobs.get(&job.space, &job.id).unwrap().unwrap();
            let status = approval.and_then(|id| {
                rusqlite::Connection::open(state.store.appdata().join("system/auth.db"))
                    .unwrap()
                    .query_row(
                        "SELECT status FROM agent_approvals WHERE id=?1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()
                    .unwrap()
            });
            (
                current.runs,
                current.checkpoint["person_request"].clone(),
                status,
            )
        })
        .await
        .unwrap()
    }
}
