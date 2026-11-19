"""Prove that an explicitly declared Draft 7 metadata title is string-valued."""

_ALL = frozenset(("null", "boolean", "array", "object", "string", "integer", "number"))
_EMPTY = frozenset()
_UNKNOWN = (_ALL, _EMPTY)
_TRIVIAL = frozenset(("$schema", "$id", "$comment", "title", "description", "default",
                     "examples", "readOnly", "writeOnly", "definitions", "$defs"))


def _intersection(left, right):
    return (left[0] & right[0], left[1] & right[1])


def _union(bounds):
    return (frozenset().union(*(bound[0] for bound in bounds)),
            frozenset().union(*(bound[1] for bound in bounds)))


def _complement(bounds):
    return (_ALL - bounds[1], _ALL - bounds[0])


def _value_type(value):
    if isinstance(value, float):
        return "integer" if value.is_integer() else "number"
    return {str: "string", bool: "boolean", int: "integer", list: "array",
            dict: "object", type(None): "null"}[type(value)]


def _literal_bounds(values):
    possible = {_value_type(value) for value in values}
    guaranteed = {"null"} & possible
    if any(value is True for value in values) and any(value is False for value in values):
        guaranteed.add("boolean")
    return frozenset(possible), frozenset(guaranteed)


def _choice(bounds, exclusive):
    may, must = _union(bounds)
    if not exclusive:
        return may, must
    possible = set()
    guaranteed = set()
    for kind in may:
        candidates = sum(kind in bound[0] for bound in bounds)
        certain = sum(kind in bound[1] for bound in bounds)
        if certain < 2:
            possible.add(kind)
        if candidates == 1 and certain == 1:
            guaranteed.add(kind)
    return frozenset(possible), frozenset(guaranteed)


def declared_title_text(metadata, title, links, resolve, errors):
    """Prove a title's primitive type through refs, literals and Draft 7 applicators.

    Track possible and guaranteed primitive types. Value constraints can narrow
    a type further. A constraint we cannot prove leaves its possible types in
    the set, so it cannot turn an unknown or mixed source into a trusted title.
    """
    def bounds(node, context, property_name, seen):
        errors.visit()
        if errors.full or len(seen) >= 96 or id(node) in seen:
            return _UNKNOWN
        if node is False:
            return (_EMPTY, _EMPTY)
        if node is True:
            return (_ALL, _ALL)
        seen = seen | {id(node)}
        if "$ref" in node:
            # The compiler recorded local scopes and relative IDs. The pinned
            # registry supplies any links inside an external schema.
            edge = links.get(id(node))
            if edge is None and context is not metadata:
                target, target_context = resolve(node["$ref"], context, errors)
                edge = (target, target_context) if target is not False else None
            # Draft 7 ignores siblings of a reference.
            return bounds(*edge, property_name, seen) if edge is not None else _UNKNOWN

        known = _TRIVIAL | {"allOf", "anyOf", "oneOf", "not", "if", "then", "else"}
        result = (_ALL, _ALL)
        if property_name is not None:
            if property_name in node.get("properties", {}):
                result = bounds(node["properties"][property_name], context, None, seen)
            # When the metadata object itself validates, its unrelated
            # constraints give no further guarantee about this property.
            known |= {"properties"}
        else:
            known |= {"type", "const", "enum"}
            if "type" in node:
                declared = node["type"]
                types = frozenset([declared] if isinstance(declared, str) else declared)
                if "number" in types:
                    types |= {"integer"}
                result = (types, types)
            if "const" in node:
                result = _intersection(result, _literal_bounds([node["const"]]))
            if "enum" in node:
                result = _intersection(result, _literal_bounds(list(errors.each(node["enum"]))))

        for child in errors.each(node.get("allOf", [])):
            result = _intersection(result, bounds(child, context, property_name, seen))
        for keyword in errors.each(("anyOf", "oneOf")):
            if keyword in node:
                choices = [bounds(child, context, property_name, seen) for child in errors.each(node[keyword])]
                # Exclusivity of metadata object branches is not determined by
                # the title alone. Use their union as an upper bound.
                result = _intersection(result, _choice(choices, keyword == "oneOf" and property_name is None))
        if "not" in node and property_name is None:
            result = _intersection(result, _complement(bounds(node["not"], context, None, seen)))
        if "if" in node:
            then = bounds(node.get("then", True), context, property_name, seen)
            otherwise = bounds(node.get("else", True), context, property_name, seen)
            if property_name is None:
                condition = bounds(node["if"], context, None, seen)
                then = _intersection(condition, then)
                otherwise = _intersection(_complement(condition), otherwise)
            result = _intersection(result, _union([then, otherwise]))
        unmodeled = node.keys() - known
        return (result[0], _EMPTY if unmodeled else result[1])

    possible, _ = bounds(metadata, metadata, title, set())
    return not errors.full and possible == {"string"}
