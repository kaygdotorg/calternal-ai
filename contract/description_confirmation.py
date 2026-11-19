"""Check preview material against the catalogue and the originating activation."""


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
    if kind == "plugin_operation":
        required.update(inputs)

    seen = set()
    for entry in errors.each(resolution["material"]):
        name = entry["name"]
        if name in seen:
            errors.append(f"$.material: duplicate confirmation entry {name!r}")
        seen.add(name)
        value = entry["value"]
        if not entry["label"].strip() or not name.strip():
            errors.append("$.material: confirmation name and label must be nonblank")
        if name in required and (value is None or value in ("", [], {}) or
                                 (isinstance(value, str) and not value.strip())):
            errors.append(f"$.material: empty confirmation value for {name!r}")
        if kind == "host.execution.resume" and name in required and not isinstance(value, str):
            errors.append(f"$.material: Resume requires a checked label for {name!r}")
    for name in errors.each(sorted(required - seen)):
        errors.append(f"$.material: missing confirmation entry {name!r}")


def check_confirmation_table(errors, catalogue):
    """Reject an unclassified operation or a row for an unknown surface."""
    policy = catalogue["confirmation"]
    challenged = set(policy["material_by_kind"])
    exempt = set(policy["no_challenge_kinds"])
    expected = set(catalogue["operations"]) | {"composer.send", "plugin_operation"}
    if challenged & exempt or challenged | exempt != expected:
        errors.append("$.confirmation: classify each host operation exactly once")
