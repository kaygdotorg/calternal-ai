"""Check preview material against the catalogue and the originating activation."""

from description_verification import check, json_equal


def check_confirmation_material(resolution, kind, facts, errors, catalogue, *, inputs=()):
    """Require named checked material for the host-derived kind and facts."""
    policy = catalogue["confirmation"]
    rule = policy["material_by_kind"].get(kind)
    if rule is None:
        errors.append(f"$.material: {kind!r} has no confirmation challenge")
        return
    if resolution.get("confirm") != "challenge":
        errors.append("$.confirm: expected a protected challenge")
        return
    required = set(rule["required"])
    for fact in errors.each(facts):
        entries = rule.get("when", {}).get(fact)
        if entries is None:
            errors.append(f"$.material: unknown checked confirmation fact {fact!r}")
            continue
        required.update(entries)
    prefix = policy["manifest_input_prefix"]
    input_keys = {prefix + name for name in inputs} if kind == "plugin_operation" else set()
    required.update(input_keys)
    check_named_material(resolution["material"], required, errors, prefix,
                         input_keys=input_keys, require_labels=kind == "host.execution.resume")


def check_named_material(material, required, errors, prefix, *, input_keys=(), require_labels=False, checked=None):
    """Check unique names, required values and the reserved manifest key space."""
    seen = set()
    for entry in errors.each(material):
        name = entry["name"]
        if name in seen:
            errors.append(f"$.material: duplicate confirmation entry {name!r}")
        if name.startswith(prefix) and name not in input_keys:
            errors.append(f"$.material: unexpected manifest input entry {name!r}")
        seen.add(name)
        value = entry["value"]
        if not entry["label"].strip() or not name.strip():
            errors.append("$.material: confirmation name and label must be nonblank")
        if name in required and (value is None or value in ("", [], {}) or
                                 (isinstance(value, str) and not value.strip())):
            errors.append(f"$.material: empty confirmation value for {name!r}")
        if require_labels and name in required and not isinstance(value, str):
            errors.append(f"$.material: Resume requires a checked label for {name!r}")
        if checked is not None and name in checked and not json_equal(value, checked[name], errors):
            errors.append(f"$.material: confirmation value for {name!r} differs from checked effect")
    for name in errors.each(sorted(required - seen)):
        errors.append(f"$.material: missing confirmation entry {name!r}")


def check_request_card_material(card, errors, catalogue, *, checked=None):
    """Validate effect material alongside decision and lifetime facts from actions."""
    policy = catalogue["confirmation"]
    rule = policy["material_by_kind"]["host.approval.resolve"]
    effect_fields = set(rule["request_card"]["effect_fields"])
    if not effect_fields <= set(checked or {}):
        errors.append("$.material: capture requires the checked request effect")
        return
    required = set(checked)
    material = list(card["material"])
    for entry in errors.each(material):
        if entry["name"] in ("decision", "lifetime"):
            errors.append(f"$.material: {entry['name']!r} must come from offered actions")
    if card["state"] == "open":
        decisions = [action["decision"] for action in errors.each(card["actions"])]
        if len(set(decisions)) != len(decisions):
            errors.append("$.actions: duplicate offered decision")
        required.update(rule["required"])
        material.append({"name": "decision", "label": "Decision", "value": decisions})
        if "approve" in decisions:
            required.update(rule["when"]["approving"])
            lifetimes = next(action["lifetimes"] for action in card["actions"] if action["decision"] == "approve")
            material.append({"name": "lifetime", "label": "Lifetime", "value": lifetimes})
    check_named_material(material, required, errors, policy["manifest_input_prefix"], checked=checked)


def check_confirmation_exchange(capture, kind, facts, errors, catalogue):
    """Check captured challenge material and bind confirmation to its exact resolve."""
    event, resolution = capture["event"], capture["resolution"]
    definition = "ResumeResolution" if kind == "host.execution.resume" else "ActivationResolution"
    check({"$ref": f"#/$defs/{definition}"}, resolution, "$.resolution", errors, catalogue)
    if errors:
        return
    if resolution["activation_id"] != event["activation_id"]:
        errors.append("$.resolution.activation_id: must match the originating invoke")
    check_confirmation_material(resolution, kind, facts, errors, catalogue)
    if errors or "confirmation" not in capture:
        return
    if kind == "composer.send":
        definition = "PreparedSendConfirmation"
    elif kind == "host.execution.resume":
        definition = "ResumeConfirmation"
    else:
        definition = "NoFinalTextConfirmation"
    confirmation = capture["confirmation"]
    check({"$ref": f"#/$defs/{definition}"}, confirmation, "$.confirmation", errors, catalogue)
    if not errors:
        for field in ("activation_id", "resolution_ref", "confirmation_challenge"):
            if confirmation[field] != resolution[field]:
                errors.append(f"$.confirmation.{field}: must match the originating resolution")


def check_confirmation_table(errors, catalogue):
    """Reject an unclassified operation or a row for an unknown surface."""
    policy = catalogue["confirmation"]
    challenged = set(policy["material_by_kind"])
    exempt = set(policy["no_challenge_kinds"])
    expected = set(catalogue["operations"]) | {"composer.send", "plugin_operation"}
    if challenged & exempt or challenged | exempt != expected:
        errors.append("$.confirmation: classify each host operation exactly once")
