# Plugin API v2 fixture refresh

This inventory records the fixture refresh for the 14.43 draft. The
normative catalogue and schemas are unchanged.

## Provenance and preserved coverage

On dev-01, built the Threads app and its separate harness from
[ae428ee6](https://git.kayg.org/kayg/calternal-ai/src/commit/ae428ee60a05c5711e34cd178d8604d24633ed21)
with its pinned 14.43 contract (`8429948b`). Ran the branch's plugin build
with `--conformance`. Copied the emitted app template and both manifests
without editing their bytes. The full fixture runner checks the template
digest and loads both manifests. The fixtures qualify wire shapes.

Scoped windows, nested personal layout, provider marks, member display
records and host archive captures have independent fixtures. The
indexed-item fixture carries those cases because the emitted Threads app
does not yet describe those Binds. Surviving schema negatives still
assert their specific error. Checked item-view pages, follow, file asset
ranges and one-shot alarms exercise the current exchanges.

The separate operation-request and note fixtures keep their own protocol
versions.

## Retired and relocated paths

Paths below are relative to `contract/fixtures/description-v2`.

The native composer challenge, options and action-scope wire were removed.

- `engines/composer-model-options.json`
- `engines/composer-view-scope.json`
- `engines/prepared-send-confirm.json`

The common host display envelope was removed; contributions use validated output and item views.

- `engines/invalid-display-control-frame.json`

The host Resume wire was removed; the app owns session continuation.

- `engines/invalid-resume-confirmation-final-text.json`
- `engines/invalid-resume-material-duplicate.json`
- `engines/invalid-resume-material-empty.json`
- `engines/invalid-resume-material-partial.json`
- `engines/invalid-resume-outcome-undo.json`
- `engines/resume-confirmation.json`
- `engines/resume-outcome-pending.json`
- `engines/resume-outcome-retired.json`
- `engines/resume-outcome-stale.json`
- `engines/resume-resolution.json`

Host transcript pages, follow, bodies and attachments were replaced by item views and checked assets.

- `engines/invalid-transcript-attachment-follow-header-linebreak.json`
- `engines/invalid-transcript-attachment-page-header-linebreak.json`
- `engines/invalid-transcript-attachments-follow-overcount.json`
- `engines/invalid-transcript-attachments-follow-oversize.json`
- `engines/invalid-transcript-attachments-follow-revoked-leak.json`
- `engines/invalid-transcript-attachments-page-overcount.json`
- `engines/invalid-transcript-attachments-page-oversize.json`
- `engines/invalid-transcript-attachments-page-revoked-leak.json`
- `engines/invalid-transcript-body-range-as-ref.json`
- `engines/invalid-transcript-both-directions.json`
- `engines/invalid-transcript-older-for-empty-cursor.json`
- `engines/invalid-transcript-record-with-empty-cursor.json`
- `engines/transcript-attachments-follow-all-revoked.json`
- `engines/transcript-attachments-follow-one-revoked.json`
- `engines/transcript-attachments-follow.json`
- `engines/transcript-attachments-page-all-revoked.json`
- `engines/transcript-attachments-page-one-revoked.json`
- `engines/transcript-attachments-page.json`
- `engines/transcript-body-final.json`
- `engines/transcript-body-page.json`
- `engines/transcript-first-commit-update.json`
- `engines/transcript-follow-update.json`
- `engines/transcript-page-empty.json`
- `engines/transcript-with-notices.json`

Moved to file-widening under engines. The same confirmation uses the generic file disclosure.

- `engines/transcript-widening-confirmation-missing-disclosure.json`
- `engines/transcript-widening-confirmation.json`
- `engines/transcript-widening-undo-confirmation.json`

The host transcript page route was removed; item_view.open serves pages.

- `event-catalogue-transcript-page.json`

Moved to events/indexed-items-app. The indexed-item fixture owns the same search and request-card cases. Archive activation sequences use captures/indexed-items-app because they are checked exchange captures, distinct from individual wire events.

- `events/threads-app/event-catalogue-component-request-card-read.json`
- `events/threads-app/event-catalogue-host-archive-confirmation.json`
- `events/threads-app/event-catalogue-host-archive-first-window.json`
- `events/threads-app/event-catalogue-search-generation-window.json`
- `events/threads-app/event-catalogue-search-page-boundary-first.json`
- `events/threads-app/event-catalogue-search-page-boundary-next.json`
- `events/threads-app/event-catalogue-search-threads.json`
- `events/threads-app/event-catalogue-threads-app-search-window-depth-200.json`
- `events/threads-app/event-catalogue-threads-app-search-window-depth-400.json`
- `events/threads-app/expected-errors.json`
- `events/threads-app/invalid-event-catalogue-component-request-card-read.json`
- `events/threads-app/invalid-event-catalogue-host-archive-confirmation-activation_id.json`
- `events/threads-app/invalid-event-catalogue-host-archive-confirmation-confirmation_challenge.json`
- `events/threads-app/invalid-event-catalogue-host-archive-confirmation-final-text.json`
- `events/threads-app/invalid-event-catalogue-host-archive-confirmation-resolution_ref.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-audience.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-bind.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-grant.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-lease.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-row-alias.json`
- `events/threads-app/invalid-event-catalogue-host-archive-first-window-row.json`
- `events/threads-app/invalid-event-catalogue-host-archive-view-context-for-row.json`
- `events/threads-app/invalid-event-catalogue-search-missing-generation.json`

The native composer or transcript component and its host-owned exchanges were removed.

- `events/threads-app/event-catalogue-component-transcript-attachment.json`
- `events/threads-app/event-catalogue-component-transcript-body.json`
- `events/threads-app/event-catalogue-component-transcript-follow.json`
- `events/threads-app/event-catalogue-component-transcript-quote.json`
- `events/threads-app/event-catalogue-composer-chain-changed-grant-and-lease.json`
- `events/threads-app/event-catalogue-composer-issued-resume.json`
- `events/threads-app/event-catalogue-composer-issued-send.json`
- `events/threads-app/event-catalogue-composer-resume-challenge.json`
- `events/threads-app/event-catalogue-composer-resume-confirmation.json`
- `events/threads-app/event-catalogue-composer-send-confirmation.json`
- `events/threads-app/event-catalogue-composer-starting-stop-after-renewal.json`
- `events/threads-app/event-catalogue-composer-stop-confirmation.json`
- `events/threads-app/invalid-event-catalogue-component-transcript-attachment.json`
- `events/threads-app/invalid-event-catalogue-component-transcript-body.json`
- `events/threads-app/invalid-event-catalogue-component-transcript-follow.json`
- `events/threads-app/invalid-event-catalogue-component-transcript-quote.json`
- `events/threads-app/invalid-event-catalogue-composer-chain-stale-grant.json`
- `events/threads-app/invalid-event-catalogue-composer-on-card.json`
- `events/threads-app/invalid-event-catalogue-composer-renewal-retired-context.json`
- `events/threads-app/invalid-event-catalogue-composer-renewal-stop-not-mapped.json`
- `events/threads-app/invalid-event-catalogue-composer-renewal-superseded.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-challenge-partial.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-client-binding.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-confirmation-activation_id.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-confirmation-confirmation_challenge.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-confirmation-final-text.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-confirmation-resolution_ref.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-confirmation-without-resolution.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-guest.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-shortcut.json`
- `events/threads-app/invalid-event-catalogue-composer-resume-wrong-context.json`
- `events/threads-app/invalid-event-catalogue-composer-send-confirmation-activation_id.json`
- `events/threads-app/invalid-event-catalogue-composer-send-confirmation-confirmation_challenge.json`
- `events/threads-app/invalid-event-catalogue-composer-send-confirmation-final-text.json`
- `events/threads-app/invalid-event-catalogue-composer-send-confirmation-resolution_ref.json`
- `events/threads-app/invalid-event-catalogue-composer-stop-confirmation-activation_id.json`
- `events/threads-app/invalid-event-catalogue-composer-stop-confirmation-confirmation_challenge.json`
- `events/threads-app/invalid-event-catalogue-composer-stop-confirmation-final-text.json`
- `events/threads-app/invalid-event-catalogue-composer-stop-confirmation-resolution_ref.json`
- `events/threads-app/invalid-event-catalogue-composer-unissued-id.json`
- `events/threads-app/invalid-event-catalogue-composer-wrong-context.json`

The host Resume intent was removed; generic job/archive races remain covered.

- `intent-races/invalid-protected-resume-archive-first.json`
- `intent-races/invalid-protected-resume-archive-undo.json`
- `intent-races/protected-resume-archive-first.json`
- `intent-races/protected-resume-archive-undo.json`

Native prepared Send in the host composer was removed.

- `invalid-authored-prepared-send.json`
- `invalid-component-no-send.json`

Sidecar of the obsolete native prepared Send case.

- `manifests/authored-prepared-send.json`

Native conversation capability and prepared composer admission have been removed.

- `manifests/install/invalid-composer-job-accepts-intent.json`
- `manifests/install/invalid-composer-job-no-intent.json`
- `manifests/install/invalid-composer-send-schedule.json`
- `manifests/install/invalid-conversation-room.json`

The hand-authored host conversation owner was superseded by the emitted Threads manifest.

- `manifests/rev14-threads-owner.json`

The verifier unit suite also retires four tests of removed surfaces.

- `test_installed_provider_id_is_unique_across_packages` tested the removed native provider registry. The installed harness case exercises the qualified contribution identity.
- `test_composer_options_requires_declared_control` tested the deleted native composer options route. Checked choice slots and component routes retain their separate tests.
- `test_composer_context_cannot_alias_any_view_or_row_action` and `test_composer_context_has_one_binding_across_renewals` covered the removed composer action map. Current archive captures retain row-versus-view context and lease renewal checks.

The composer-specific renewal matrix is recast as a multiple-renewal archive test.
