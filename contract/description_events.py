"""Template-aware checks for event identifiers and catalogue request shapes."""

from description_verification import check

COMPONENT_TYPES = {"item-editor", "turn-transcript", "composer", "request-card", "sealed-html-frame"}
TYPED_ROUTES = {
    "transcript.page": "turn-transcript",
    "composer.options": "composer",
    "request_resolve": "request-card",
    "sealed_frame.open": "sealed-html-frame",
    "sealed_frame.resize": "sealed-html-frame",
    "sealed_frame.open_link": "sealed-html-frame",
}
HOST_SURFACE_ROUTES = {"undo", "status", "view.open", "view.renew"}


def walk(node, errors, path="$"):
    """Yield each JSON node with its document path."""
    if errors.full:
        return
    errors.visit()
    yield path, node
    if isinstance(node, dict):
        for key, value in errors.each(node.items()):
            yield from walk(value, errors, f"{path}.{key}")
    elif isinstance(node, list):
        for index, value in enumerate(errors.each(node)):
            yield from walk(value, errors, f"{path}[{index}]")


def prompt_slots(action, errors):
    """Index the device-held prompt and choice slots by their input names."""
    inputs = action.get("inputs", {})
    return {name: ("choices" if operand["prompt"].get("kind") == "choice" else "prompt")
            for name, operand in errors.each(inputs.items()) if "prompt" in operand}


class EventBindings:
    """Index the authored identifiers of one verified app template."""

    def __init__(self, view, errors):
        chrome = view.get("chrome", {})
        self.sidebar = chrome.get("sidebar")
        self.panel = chrome.get("panel")
        footer = chrome.get("footer", {})
        self.apps = {app["id"] for app in errors.each(footer.get("apps", []))}
        self.footer_search = footer.get("search", {}).get("bind")
        self.sections = {section["id"]: section for section in errors.each((self.sidebar or {}).get("sections", []))}
        self.binds = {bind["id"]: bind for bind in errors.each(view.get("binds", []))}
        self.actions = {}
        self.shell_actions = {}
        self.components = {}
        self.routes = set()
        self.row_routes = set()
        self.route_values = {}
        for selector in errors.each((self.sidebar or {}).get("selectors", [])):
            if "route" in selector:
                self.route_values.setdefault(selector["route"], set()).update(
                    option["id"] for option in errors.each(selector["options"])
                )
        for section in errors.each(self.sections.values()):
            for shortcut in errors.each(section.get("rows", [])):
                opened = shortcut.get("open", {})
                route = opened.get("route") or opened.get("invoke", {}).get("shell", {}).get("route", {})
                literal = route.get("value", {}).get("literal")
                if isinstance(literal, str):
                    self.route_values.setdefault(route["name"], set()).add(literal)
        self.form_fields = {}
        self.choices = set()
        self.multiple = set()
        for _, node in walk(view, errors):
            if not isinstance(node, dict):
                continue
            if isinstance(route := node.get("route"), str):
                self.routes.add(route)
            elif isinstance(route, dict) and isinstance(route.get("name"), str):
                self.routes.add(route["name"])
            kind = node.get("type")
            if kind in COMPONENT_TYPES:
                self.components[node["id"]] = node
            if kind == "form":
                self.form_fields[node["id"]] = {field["id"] for field in errors.each(node["fields"])}
            if kind == "list" and node.get("selectionMode") == "multiple":
                self.multiple.add(node["bind"])
            if isinstance(node.get("operation"), str) and "id" in node:
                self.actions[node["id"]] = node
                self.choices.update(name for name, slot in errors.each(prompt_slots(node, errors).items()) if slot == "choices")
            if "shell" in node:
                self.shell_actions[node["id"]] = node["shell"]
        for bind in errors.each(self.binds.values()):
            route = bind.get("row", {}).get("open", {}).get("route")
            if route:
                self.row_routes.add(route["name"])

    def bind_for(self, ident, errors):
        """Resolve an event's Bind ID within the originating template."""
        errors.visit()
        bind = self.binds.get(ident)
        if bind is None:
            errors.append(f"$: bind {ident!r} is not in the active description")
        return bind


def check_invoke(event, bindings, errors):
    """Check action-local device values and bulk row scope against the invoke."""
    errors.visit()
    action = bindings.actions.get(event["action_id"])
    if action is None:
        errors.append(f"$.action_id: unknown action {event['action_id']!r} in the active description")
        return
    inputs = action.get("inputs", {})
    slots = prompt_slots(action, errors)
    fields = set()
    for operand in errors.each(inputs.values()):
        if "form" in operand and operand["field"] in bindings.form_fields.get(operand["form"], ()):
            fields.add(operand["field"])
    interaction = event.get("interaction", {})
    for field in errors.each(interaction.get("form", {})):
        if field not in fields:
            errors.append(f"$.interaction.form: unknown form field {field!r} for this action")
    for group, description in errors.each((("prompt", "prompt"), ("choices", "choice"))):
        for slot in errors.each(interaction.get(group, {})):
            if slots.get(slot) != group:
                errors.append(f"$.interaction.{group}: unknown {description} slot {slot!r} for this action")
    operands = inputs.values()
    if {"rows", "selection_ref"} & interaction.keys() and not any(
        operand.get("set") is True and operand.get("selection") in bindings.multiple for operand in errors.each(operands)
    ):
        errors.append("$.interaction: row selection requires this action's multiple-selection Bind")
    if {"draft", "options"} & interaction.keys() and not any("draft" in operand for operand in errors.each(operands)):
        errors.append("$.interaction: draft and options require this action's composer")


def validate_component_request(event, errors, catalogue):
    """Check the route-specific input shape before the template index is available."""
    if catalogue is None:
        return
    errors.visit()
    route = event["route"]
    entry = catalogue["component_routes"].get(route)
    if entry is None:
        errors.append(f"$.route: unknown catalogue component route {route!r}")
        return
    check(entry["input_schema"], event["request"], "$.request", errors, catalogue)


def check_component(event, bindings, errors, catalogue):
    """Check a shaped route request against its declared component, Bind or prompt."""
    if bindings is None or catalogue is None:
        return
    errors.visit()
    route = event["route"]
    request = event["request"]
    ident = event["component"]
    component = bindings.components.get(ident)
    shell = bindings.shell_actions.get(ident)
    action = bindings.actions.get(ident)
    if route == "selection.set":
        selection_bind = bindings.sections.get(ident, {}).get("bind", ident)
        if ident not in bindings.sections and ident not in bindings.binds:
            errors.append(f"$.component: unknown component {ident!r} in the active description")
        elif request["bind"] != selection_bind:
            errors.append("$.request.bind: selection does not name this component's Bind")
        else:
            check_event_binding(request, bindings, errors, catalogue)
        return

    if component is None and shell is None and action is None:
        errors.append(f"$.component: unknown component {ident!r} in the active description")
        return
    allowed = route in HOST_SURFACE_ROUTES
    if route in TYPED_ROUTES:
        allowed = component is not None and component["type"] == TYPED_ROUTES[route]
        if allowed and route == "composer.options":
            controls = component["config"].get("controls", [])
            if request["control"] not in errors.each(controls):
                errors.append(f"$.request.control: control {request['control']!r} is not configured for {ident!r}")
    elif route == "choice.options":
        allowed = action is not None and prompt_slots(action, errors).get(request["slot"]) == "choices"
    elif route in ("share_sheet", "trash_sheet"):
        allowed = shell is not None and shell.get("sheet") == route.removesuffix("_sheet")
        if allowed and route == "trash_sheet" and request["kind"] not in {
            bind["source"]["kind"] for bind in errors.each(bindings.binds.values())
        }:
            errors.append("$.request.kind: Trash Sheet kind is not declared by a Bind")
    if not allowed:
        errors.append(f"$.component: {ident!r} cannot use route {route!r}")


def check_event_binding(event, bindings, errors, catalogue):
    """Check all template-defined identifiers. Host-issued references stay opaque."""
    errors.visit()
    event_type = event["type"]
    if event_type == "component":
        check_component(event, bindings, errors, catalogue)
    elif bindings is None:
        return
    elif event_type == "invoke":
        check_invoke(event, bindings, errors)
    elif event_type == "window":
        request = event["request"]
        bind = bindings.bind_for(request["bind"], errors)
        if bind is not None:
            if ("parent" in request) != bool(bind["source"].get("parent")):
                errors.append("$.request.parent: window parent scope must match source.parent")
            if bind["source"]["type"] == "search":
                check({"$ref": "#/$defs/EventSearchWindow"}, event, "$", errors)
    elif event_type in ("selection", "selection_set"):
        bind = bindings.bind_for(event["bind"], errors)
        if bind and bind["source"]["type"] not in ("list", "search"):
            errors.append("$.bind: selection requires a list or search Bind")
        if bind and event_type == "selection_set" and event["bind"] not in bindings.multiple:
            errors.append("$.bind: a selection set requires a multiple-selection Bind")
    elif event_type == "search":
        bind = bindings.bind_for(event.get("bind", bindings.footer_search), errors)
        if bind and bind["source"]["type"] != "search":
            errors.append(f"$: bind {bind['id']!r} is not a search bind")
    elif event_type == "refresh":
        if "bind" in event:
            bindings.bind_for(event["bind"], errors)
    elif event_type == "route":
        if event["id"] not in bindings.routes:
            errors.append(f"$.id: unknown route {event['id']!r} in the active description")
        elif ("row" in event and event["id"] not in bindings.row_routes
              and event["row"] not in bindings.route_values.get(event["id"], ())):
            errors.append("$.row: route has no matching row scope, selector option or shortcut value")
    elif event_type == "section":
        if event["id"] not in bindings.sections:
            errors.append(f"$.id: unknown section {event['id']!r} in the active description")
    elif event_type == "layout":
        panel = (bindings.panel or {}).get("id")
        if not (event["id"] == panel or (event["id"] == "sidebar" and bindings.sidebar is not None)):
            errors.append(f"$.id: unknown layout {event['id']!r} in the active description")
    elif event_type == "sidebar":
        if bindings.sidebar is None:
            errors.append("$: the active description has no sidebar")
    elif event_type == "app":
        if event["id"] not in bindings.apps:
            errors.append(f"$.id: unknown app {event['id']!r} in the active description")
    elif event_type == "choice_options":
        if event["slot"] not in bindings.choices:
            errors.append(f"$.slot: unknown choice slot {event['slot']!r} in the active description")
    elif event_type not in {"invoke_confirm", "invoke_dispatch", "view_open", "lease_renew", "undo", "action_status"}:
        errors.append(f"$.type: missing template checker for event {event_type!r}")
