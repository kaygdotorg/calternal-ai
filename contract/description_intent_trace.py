"""Check execution qualification traces before a runtime gate is available."""


def check_discovery_audience(case, errors, catalogue):
    """Compare the discovery inputs against the fixture's live read grants."""
    policy = catalogue["execution_attempt_authority"]["discovery"]["transcript_audience"]
    reader_sets = {"sender": [case["sender"]], "transcript_readers": case["transcript_readers"]}
    required = {reader for name in policy["required_reader_sets"] for reader in reader_sets[name]}
    visible = sorted(name for name, readers in case["input_readers"].items() if required <= set(readers))
    for operation in catalogue["execution_attempt_authority"]["discovery"]["operations"]:
        if sorted(case["observed"][operation]) != visible:
            errors.append(f"$.observed.{operation}: discovery exceeds or omits the checked transcript audience")


def check_intent_trace(trace, errors, catalogue):
    """Verify that the recorded outcomes obey the same fence for either origin."""
    if trace["origin"] not in catalogue["execution_intents"]["origins"]:
        errors.append("$.origin: unknown execution intent origin")
        return
    archived = False
    pending = None
    admitted = set()
    active = None
    refused = set()
    for index, event in enumerate(errors.each(trace["events"])):
        operation, intent, observed = event
        if operation == "commit_intent":
            if archived or active is not None or pending is not None or intent in refused or intent in admitted:
                expected = "refused"
            else:
                expected = "starting"
                pending = intent
        elif operation == "idle_archive":
            expected = "challenge" if pending is not None or active is not None else "archived"
            if expected == "archived":
                archived = True
        elif operation == "confirmed_archive":
            if pending is None or pending != intent:
                expected = "stale"
            else:
                archived = True
                refused.add(intent)
                pending = None
                expected = "archived_and_refused"
        elif operation == "undo":
            expected = "restored" if archived else "stale"
            archived = False
        elif operation == "restart":
            expected = "recovered"
        elif operation == "claim":
            if archived or intent in refused:
                expected = "refused"
            elif intent in admitted:
                expected = "replayed"
            elif pending == intent:
                expected = "started"
                admitted.add(intent)
                pending = None
                active = intent
            else:
                expected = "refused"
        else:
            errors.append(f"$.events[{index}]: unknown intent trace operation {operation!r}")
            continue
        if observed != expected:
            errors.append(f"$.events[{index}]: expected {expected!r} at the execution intent fence")


def check_attempt_fence_trace(trace, errors, catalogue):
    """Check crash probes with the remote still running and its Stop withheld."""
    action = trace["action"]
    rule = catalogue["attempt_authority_end"]["actions"].get(action)
    if rule is None:
        errors.append("$.action: unknown attempt authority action")
        return
    ends_authority = rule["ends_authority"]
    fenced = rule.get("fences_effects", ends_authority)
    cutover = action == "cutover_retirement"
    before = {"fence_durable": fenced, "cleanup_durable": rule.get("cleanup_durable", ends_authority),
              "reply_sent": False, "remote_stops": 0}
    # Cutover recovers already-persisted cleanup for an earlier authorized
    # retirement. It has no active attempt to stop as part of the switch.
    if cutover:
        before["cleanup_durable"] = False
        before["prior_cleanup_durable"] = True
    if trace["after_commit"] != before:
        errors.append("$.after_commit: wrong durable fence, cleanup or reply state before the crash")
    expected_probes = {name: "refused" if fenced else "allowed"
                       for name in ("renewal", "broker_effect", "queued_tool", "egress", "descendant_effect")}
    for point in ("before_dispatch", "after_restart_before_dispatch"):
        if trace[point] != expected_probes:
            errors.append(f"$.{point}: wrong authority while remote Stop is withheld")
    if action == "archive" and trace.get("after_undo_before_dispatch") != expected_probes:
        errors.append("$.after_undo_before_dispatch: archive Undo reopened fenced authority")
    if action == "conversation_audience_widening":
        expected = {"new_reader_committed_transcript": "allowed", "personal_uncommitted_output": "discarded"}
        for point in ("widened_after_commit", "widened_after_restart"):
            if trace.get(point) != expected:
                errors.append(f"$.{point}: widening must expose only the committed transcript boundary")
        if trace.get("later_run") != {"discovery": "whole_transcript_audience", "personal_login_tools": "disabled"}:
            errors.append("$.later_run: subsequent attempts must follow the shared rules")
    if action in ("failed_reattachment", "runtime_freeze") and trace.get("stale_thaw") != "refused":
        errors.append("$.stale_thaw: old proof cannot reopen the runtime gate")
    if action == "failed_reattachment" and trace.get("placement_lease") != "still_valid":
        errors.append("$.placement_lease: failed continuity must fence even a valid placement")
    if trace["unaffected_attempt"] != "allowed":
        errors.append("$.unaffected_attempt: retirement cannot fence unrelated authority")
    expected_stops = int(rule.get("new_remote_stop", ends_authority and not cutover))
    expected_cleanup = {"new_exact_stops": expected_stops, "replacement_stops": 0}
    if cutover:
        expected_cleanup["prior_exact_stops"] = 1
    if trace["after_cleanup_replay"] != expected_cleanup:
        errors.append("$.after_cleanup_replay: remote dispatch must reconcile the exact attempt once")
