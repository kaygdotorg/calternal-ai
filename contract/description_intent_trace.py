"""Check archive/Undo qualification traces before a runtime gate is available."""


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
