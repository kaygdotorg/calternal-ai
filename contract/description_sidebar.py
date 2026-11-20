"""Sidebar structure checks the schema cannot say: flat sections, strips, arrangement and drag hooks.

The rules follow the catalogue's `sidebar_arrangement` and description contract §2.7. Each
check walks only the sidebar sections and the Bind list once; cost is linear in the template.
"""
import re

HOOKS = ("drop", "reorder", "dragOut")
HOOK_PATH = re.compile(r"\.(drop|reorder|dragOut)\.inputs\.[A-Za-z0-9_]+\Z")
ROW_HOOK_PATH = re.compile(r"^\$\.binds\[(\d+)\]\.row\.(drop|reorder|dragOut)\.inputs\.[A-Za-z0-9_]+\Z")


def sidebar_sections(doc, errors):
    """Yield (path, section) for every sidebar section; sections are flat (no subsections)."""
    sections = (((doc.get("chrome") or {}).get("sidebar") or {}).get("sections")) or []
    for index, section in enumerate(errors.each(sections)):
        if isinstance(section, dict):
            yield f"$.chrome.sidebar.sections[{index}]", section


def repeat_over_binds(doc, errors):
    """Map each `repeat.over` Bind id to the paths of the repeats that use it."""
    found = {}
    for path, section in sidebar_sections(doc, errors):
        over = (section.get("repeat") or {}).get("over")
        if isinstance(over, str):
            found.setdefault(over, []).append(path)
    return found


def dropped_source(source):
    """Return the `dropped` value of a ValueSource, or None."""
    if isinstance(source, dict) and source.keys() == {"dropped"}:
        return source["dropped"]
    return None


def dropped_ref_type(source, path):
    """The ref type a `{dropped}` operand supplies at this hook path, or None when it is not one."""
    value = dropped_source(source)
    if value == "item":
        return {"ItemRef"}
    if value == "placement":
        return {"LayoutEntryRef"}
    if value == "before":
        return {"LayoutEntryRef"} if ROW_HOOK_PATH.search(path or "") else {"ItemRef"}
    return None


def folder_destination(path, binds_list, over_binds):
    """True when an input at `path` sits in an own-section row hook, where `{row: id}` is that section's LayoutSectionRef."""
    match = ROW_HOOK_PATH.search(path or "")
    if not match:
        return False
    index = int(match.group(1))
    return index < len(binds_list) and binds_list[index].get("id") in over_binds


def check_sidebar(doc, errors, binds, binds_list):
    """Check the single repeat, strips, arrangement, hook placement and `{dropped}` scope."""
    over_binds = repeat_over_binds(doc, errors)
    repeats = [path for paths in errors.each(over_binds.values()) for path in errors.each(paths)]
    if len(repeats) > 1:
        errors.append(f"{sorted(repeats)[1]}: a description declares at most one `repeat` (the person's own sections, ordered among the others)")
    for path, section in sidebar_sections(doc, errors):
        if (section.get("arrange") or {}).get("rename") and "title" not in section:
            errors.append(f"{path}: arrange.rename needs a `title` to rename")
        if section.get("presentation") == "strip":
            check_strip(path, section, binds, errors)
        hooks = [key for key in HOOKS if key in section]
        if hooks and "bind" not in section:
            errors.append(f"{path}: {', '.join(hooks)} needs the section's own `bind` (hooks for the person's own sections belong in the `repeat.over` row template)")
        if "reorder" in section and "bind" in section:
            check_reorder(path, section["reorder"], binds.get(section["bind"]), errors)
    for index, bind in enumerate(errors.each(binds_list)):
        row = bind.get("row") or {}
        hooks = [key for key in HOOKS if key in row]
        if hooks and bind.get("id") not in over_binds:
            errors.append(f"$.binds[{index}].row: {', '.join(hooks)} in a row template is legal only for a `repeat.over` bind (the header of one of the person's own sections)")
    check_dropped_scope(doc, errors)


def check_strip(path, section, binds, errors):
    """A strip draws one Bind's rows as round marks: no other content, avatar icons, no row extras."""
    for key in errors.each(("rows", "repeat")):
        if key in section:
            errors.append(f"{path}: a strip has no `{key}`")
    bind = binds.get(section.get("bind"))
    if bind is None:
        errors.append(f"{path}: a strip needs a `bind`")
        return
    row = bind.get("row") or {}
    if not (isinstance(row.get("icon"), dict) and "avatar" in row["icon"]):
        errors.append(f"{path}: a strip's bind {section['bind']!r} draws a RowAvatarIcon (`icon.avatar`)")
    for key in errors.each(("children", "secondary", "leadingActions", "trailingActions")):
        if key in row:
            errors.append(f"{path}: a strip's rows have no `{key}`")


def check_reorder(path, hook, bind, errors):
    """A host.viewer.reorder hook writes the field its section's Bind sorts by first, ascending."""
    if not isinstance(hook, dict) or hook.get("operation") != "host.viewer.reorder" or bind is None:
        return
    field = ((hook.get("inputs") or {}).get("field") or {}).get("literal")
    first = (bind.get("sort") or [{}])[0]
    if not isinstance(field, str) or first.get("field") != field or first.get("direction", "asc") != "asc":
        errors.append(f"{path}.reorder: host.viewer.reorder needs a literal `field` that the section's bind sorts by first, ascending")


def check_dropped_scope(doc, errors):
    """`{dropped}` operands are legal only inside hooks; `before` only in reorder; `placement` only in own-section hooks."""
    for path, node in errors.each(walk_inputs(doc, errors)):
        value = dropped_source(node)
        if value is None:
            continue
        hook = HOOK_PATH.search(path)
        if not hook:
            errors.append(f"{path}: {{dropped}} is legal only in a drop, reorder or dragOut hook")
        elif value == "before" and hook.group(1) != "reorder":
            errors.append(f"{path}: {{dropped: before}} is legal only in a reorder hook")
        elif value == "placement" and not ROW_HOOK_PATH.search(path):
            errors.append(f"{path}: {{dropped: placement}} is legal only in a hook of the person's own sections (a `repeat.over` row template)")


def walk_inputs(doc, errors, path="$"):
    """Yield (path, operand) for every invoke input value in the document."""
    stack = [(path, doc)]
    while stack and not errors.full:
        errors.visit()
        here, node = stack.pop()
        if isinstance(node, dict):
            inputs = node.get("inputs")
            if isinstance(inputs, dict) and "operation" in node:
                for name, operand in inputs.items():
                    yield f"{here}.inputs.{name}", operand
            stack.extend((f"{here}.{key}", value) for key, value in node.items() if key != "literal")
        elif isinstance(node, list):
            stack.extend((f"{here}[{index}]", value) for index, value in enumerate(node))
