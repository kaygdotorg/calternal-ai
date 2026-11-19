"""Check the canonical declaration of a kind's generic title."""


def declared_title_text(metadata, title, links, errors):
    """Read the named property and follow references within its metadata document."""
    if not isinstance(metadata, dict) or "$ref" in metadata:
        return False
    node = metadata.get("properties", {}).get(title)
    seen = set()
    while isinstance(node, dict) and "$ref" in node:
        errors.visit()
        if errors.full or id(node) in seen:
            return False
        seen.add(id(node))
        target, document = links.get(id(node), (None, None))
        if document is not metadata:
            return False
        node = target
    return isinstance(node, dict) and node.get("type") in ("string", ["string"])
