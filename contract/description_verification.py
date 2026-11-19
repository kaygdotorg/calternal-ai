"""Pinned schema, manifest and window-reply validation for the plugin v2 verifier."""

import os
import pathlib
import re
from datetime import datetime
from functools import cache
from itertools import chain
from urllib.parse import unquote, urldefrag, urljoin, urlsplit

from description_input import DuplicateKey, VisitLimit, check_unique_ids, load_strict, read_bytes, read_json, resolved_path


root = pathlib.Path(__file__).resolve().parent
schema = read_json(root / "description-v2.schema.json")
manifest_schema = read_json(root / "plugin-manifest-v2.schema.json")
legacy_manifest_schema = read_json(root / "plugin-manifest-v1.schema.json")
draft7_meta = read_json(root / "json-schema-draft-07.json")

CATALOGUE = read_json(root / "host-catalogue-v2.json")
REGISTRY_BASE = "https://calternal.cloud/contract/"


def schema_registry(catalogue):
    """Return the checked-in schema resources and their registered names."""
    documents = {
        "host-catalogue-v2.json": catalogue,
        "description-v2.schema.json": schema,
        "plugin-manifest-v2.schema.json": manifest_schema,
        "plugin-manifest-v1.schema.json": legacy_manifest_schema,
    }
    return {**documents, **{REGISTRY_BASE + name: doc for name, doc in documents.items()}}


SCHEMA_DOCUMENTS = schema_registry(CATALOGUE)


def use_fixture_catalogue():
    """Permit diagnostic catalogue overrides only from the explicit fixture mode."""
    global CATALOGUE, SCHEMA_DOCUMENTS
    if override := os.environ.get("CALTERNAL_HOST_CATALOGUE"):
        path = pathlib.Path(override)
        CATALOGUE = read_json(path) if path.exists() else None
        SCHEMA_DOCUMENTS = schema_registry(CATALOGUE)
        pinned_schema_positions.cache_clear()


def pointer_tokens(fragment, errors):
    """Decode a URI fragment into exact JSON Pointer path components."""
    if re.search(r"%(?![0-9A-Fa-f]{2})", fragment):
        raise ValueError("malformed percent escape")
    pointer = unquote(fragment, errors="strict")
    if not pointer:
        return ()
    if not pointer.startswith("/"):
        raise ValueError("expected a JSON Pointer")
    tokens = []
    for token in errors.each(pointer[1:].split("/")):
        if re.search(r"~(?![01])", token):
            raise ValueError("malformed JSON Pointer escape")
        tokens.append(token.replace("~1", "/").replace("~0", "~"))
    return tuple(tokens)


def pointer_target(document, fragment, errors):
    """Resolve a URI fragment as a JSON Pointer without changing its document."""
    node = document
    for part in errors.each(pointer_tokens(fragment, errors)):
        if isinstance(node, list):
            if not re.fullmatch(r"0|[1-9][0-9]*", part):
                raise ValueError("invalid JSON Pointer array index")
            node = node[int(part)]
        else:
            node = node[part]
    return node


def resolve(ref, ctx, errors):
    """Resolve a JSON Pointer in the current document or pinned offline registry."""
    errors.visit()
    base, separator, fragment = ref.partition("#")
    if not separator:
        return False, ctx
    if base:
        ctx = SCHEMA_DOCUMENTS.get(base)
    if ctx is None:
        return False, schema
    try:
        return pointer_target(ctx, fragment, errors), ctx
    except VisitLimit:
        raise
    except (KeyError, IndexError, TypeError, ValueError):
        return False, ctx


def is_date_time(value):
    """Parse calendar dates and zones after the catalogue checks RFC 3339 syntax."""
    try:
        timestamp = datetime.fromisoformat(value[:-1] + "+00:00" if value.endswith(("Z", "z")) else value)
    except ValueError:
        return False
    return timestamp.tzinfo is not None


def json_equal(left, right, errors):
    """Compare JSON values without treating booleans as numbers."""
    errors.visit()
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is type(right) and left == right
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(json_equal(a, b, errors) for a, b in errors.each(zip(left, right)))
    if isinstance(left, dict) and isinstance(right, dict):
        return len(left) == len(right) and all(
            key in right and json_equal(value, right[key], errors)
            for key, value in errors.each(left.items())
        )
    return left == right


def json_key(value, errors):
    """Hash one JSON value with the same equality rules as the schema."""
    errors.visit()
    if isinstance(value, dict):
        return frozenset((key, json_key(child, errors)) for key, child in errors.each(value.items()))
    if isinstance(value, list):
        return tuple(json_key(child, errors) for child in errors.each(value))
    if isinstance(value, bool):
        return (bool, value)
    return value


def short_value(value):
    """Keep untrusted values in diagnostics small."""
    if isinstance(value, str):
        return repr(value[:256]) + ("…" if len(value) > 256 else "")
    return f"<{type(value).__name__}>" if isinstance(value, (dict, list)) else repr(value)


def check(node, value, path, errors, ctx=None, *, depth=0):
    """Validates `value` against `node`: the JSON Schema keywords the contract and the host catalogue use, including boolean schemas, not, if/then/else, allOf,
    anyOf, oneOf and dependentRequired (the catalogue's ActionOutcome and ActivationResolution rely on them)."""
    if errors.full:
        return
    errors.visit()
    if isinstance(value, (list, dict)):
        errors.require_visits(len(value))
    if depth > 128:
        raise ValueError("Schema validation exceeds 128 nested evaluations")

    reject = errors.append

    def descend(child, instance, child_path, context):
        check(child, instance, child_path, errors, context, depth=depth + 1)

    def matches(child, instance, context):
        if errors.full:
            return False
        with errors.trial() as found:
            descend(child, instance, path, context)
            return not found

    ctx = ctx if ctx is not None else schema
    if node is True: return
    if node is False: reject(f"{path}: not allowed here"); return
    if "$ref" in node:
        target, ctx2 = resolve(node["$ref"], ctx, errors)
        descend(target, value, path, ctx2)
        if ctx.get("$schema") == "http://json-schema.org/draft-07/schema#":
            return
    for branch in errors.each(node.get("allOf", [])): descend(branch, value, path, ctx)
    if "anyOf" in node and not any(matches(b, value, ctx) for b in errors.each(node["anyOf"])): reject(f"{path}: matches none of the anyOf branches")
    if errors.full:
        return
    if "oneOf" in node:
        results = []
        for b in errors.each(node["oneOf"]):
            with errors.trial() as sub:
                descend(b, value, path, ctx)
            results.append(sub)
        passing = sum(1 for r in results if not r)
        if passing != 1:
            closest = []
            if passing == 0:
                # Prefer the branch for the supplied discriminator over a shorter
                # error from a different event or component.
                mismatch = f"{path}.type: must be "
                tagged = [result for result in results if not any(error.startswith(mismatch) for error in result)]
                if not tagged and isinstance(value, dict) and "type" in value:
                    closest = [f"{path}.type: unsupported discriminator {short_value(value['type'])}"]
                else:
                    closest = min(tagged or results, key=len)
            reject(f"{path}: matches {passing} of the oneOf branches (needs exactly 1)" + (f"; closest branch: {'; '.join(closest[:3])}" if closest else ""))
    if "not" in node and matches(node["not"], value, ctx): reject(f"{path}: matches a forbidden shape")
    if "if" in node:
        branch = node.get("then") if matches(node["if"], value, ctx) else node.get("else")
        if branch is not None: descend(branch, value, path, ctx)
    t = node.get("type")
    kinds = {"object": dict, "array": list, "string": str, "boolean": bool, "null": type(None)}
    if t:
        ok_type = False
        for one in errors.each(t if isinstance(t, list) else [t]):
            if one == "integer": ok_type |= isinstance(value, int) and not isinstance(value, bool)
            elif one == "number": ok_type |= isinstance(value, (int, float)) and not isinstance(value, bool)
            elif one in kinds: ok_type |= isinstance(value, kinds[one]) and (one == "boolean" or not isinstance(value, bool))
        if not ok_type: reject(f"{path}: expected {t}"); return
    if "const" in node and not json_equal(value, node["const"], errors): reject(f"{path}: must be {node['const']!r}")
    if "enum" in node and not any(json_equal(value, candidate, errors) for candidate in errors.each(node["enum"])): reject(f"{path}: {short_value(value)} not in {node['enum']}")
    if isinstance(value, str):
        if node.get("format") == "date-time" and not is_date_time(value):
            reject(f"{path}: must be an RFC 3339 date-time with seconds 00 to 59")
        if "pattern" in node and not re.search(node["pattern"], value): reject(f"{path}: {short_value(value)} does not match {node['pattern']}")
        if "maxLength" in node and len(value.encode()) > node["maxLength"]: reject(f"{path}: longer than {node['maxLength']} bytes")
        if "minLength" in node and len(value) < node["minLength"]: reject(f"{path}: shorter than {node['minLength']}")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in node and value < node["minimum"]: reject(f"{path}: below {node['minimum']}")
        if "maximum" in node and value > node["maximum"]: reject(f"{path}: above {node['maximum']}")
        if "exclusiveMinimum" in node and value <= node["exclusiveMinimum"]:
            reject(f"{path}: must exceed {node['exclusiveMinimum']}")
        if "exclusiveMaximum" in node and value >= node["exclusiveMaximum"]:
            reject(f"{path}: must be below {node['exclusiveMaximum']}")
    if isinstance(value, list):
        if "minItems" in node and len(value) < node["minItems"]: reject(f"{path}: fewer than {node['minItems']} items")
        if "maxItems" in node and len(value) > node["maxItems"]: reject(f"{path}: more than {node['maxItems']} items")
        if node.get("uniqueItems"):
            if len({json_key(item, errors) for item in errors.each(value)}) != len(value):
                reject(f"{path}: items not unique")
        prefix = node.get("prefixItems", [])
        for i, item_schema in enumerate(errors.each(prefix[:len(value)])):
            descend(item_schema, value[i], f"{path}[{i}]", ctx)
        if "items" in node:
            for i in errors.each(range(len(prefix), len(value))):
                descend(node["items"], value[i], f"{path}[{i}]", ctx)
        if "contains" in node and not any(matches(node["contains"], item, ctx) for item in errors.each(value)):
            reject(f"{path}: no item matches contains")
    if isinstance(value, dict):
        for key in errors.each(node.get("required", [])):
            if key not in value: reject(f"{path}: missing {key}")
        for key, needs in errors.each(node.get("dependentRequired", {}).items()):
            if key in value:
                for other in errors.each(needs):
                    if other not in value: reject(f"{path}: {key} needs {other}")
        props = node.get("properties", {})
        extra = node.get("additionalProperties", True)
        if "maxProperties" in node and len(value) > node["maxProperties"]: reject(f"{path}: more than {node['maxProperties']} properties")
        if "minProperties" in node and len(value) < node["minProperties"]: reject(f"{path}: fewer than {node['minProperties']} properties")
        for key, v in errors.each(value.items()):
            if "propertyNames" in node:
                descend(node["propertyNames"], key, f"{path}.{key}", ctx)
            if key in props: descend(props[key], v, f"{path}.{key}", ctx)
            elif extra is False: reject(f"{path}: unknown field {key}")
            elif isinstance(extra, dict): descend(extra, v, f"{path}.{key}", ctx)


def field_names(kind_entry, errors, generic=()):
    fields = kind_entry.get("fields") or kind_entry.get("indexed_fields") or {}
    names = set(errors.each(fields)) if isinstance(fields, dict) else {
        f.get("name") if isinstance(f, dict) else f for f in errors.each(fields)
    }
    return names | set(generic)

def normalize_manifest(manifest, errors):
    """Index the declarations of a schema-validated v2 manifest by ID."""
    kinds = {kind["id"]: {**kind, "fields": kind.get("indexed_fields", {})}
             for kind in errors.each(manifest["kinds"])}
    return {**manifest, "kinds": kinds,
            "operations": {op["id"]: op for op in errors.each(manifest["operations"])},
            "jobs": {job["id"]: job for job in errors.each(manifest.get("jobs", []))}}


def schema_children(node, errors=None, *, modern=False):
    """Yield path segments for schema children, skipping annotations and data."""
    each = iter if errors is None else errors.each
    if errors is not None and errors.full:
        return
    for key in ("definitions", "$defs", "properties", "patternProperties"):
        collection = node.get(key, {})
        if isinstance(collection, dict):
            yield from (((key, name), child) for name, child in each(collection.items()))
    for key in ("additionalProperties", "additionalItems", "propertyNames", "contains", "if", "then", "else", "not"):
        if key in node:
            yield (key,), node[key]
    for key in (("allOf", "anyOf", "oneOf", "prefixItems") if modern else ("allOf", "anyOf", "oneOf")):
        children = node.get(key, [])
        if isinstance(children, list):
            yield from (((key, str(index)), child) for index, child in enumerate(each(children)))
    if "items" in node:
        items = node["items"]
        if isinstance(items, list):
            yield from ((("items", str(index)), child) for index, child in enumerate(each(items)))
        else:
            yield ("items",), items
    for key in (("dependencies", "dependentSchemas") if modern else ("dependencies",)):
        collection = node.get(key, {})
        if isinstance(collection, dict):
            yield from (((key, name), child) for name, child in each(collection.items()) if not isinstance(child, list))


def schema_positions(document):
    """Index positions in a trusted pinned document. Embedded schemas use the budgeted walk."""
    pending = [((), document)]
    modern = document.get("$schema") == "https://json-schema.org/draft/2020-12/schema"
    while pending:
        path, node = pending.pop()
        if isinstance(node, (dict, bool)):
            yield path, node
        if isinstance(node, dict):
            pending.extend((path + parts, child) for parts, child in schema_children(node, modern=modern))


@cache
def pinned_schema_positions():
    """Cache each pinned resource's schema positions once per catalogue."""
    documents = {name: doc for name, doc in SCHEMA_DOCUMENTS.items() if name.startswith("https://") and doc is not None}
    documents[draft7_meta["$id"].removesuffix("#")] = draft7_meta
    return {name: dict(schema_positions(doc)) for name, doc in documents.items()}


def schema_uri(base, ref):
    if len(ref.encode("utf-8")) > 2048 or re.search(r"[\x00-\x20\x7f]|%(?![0-9A-Fa-f]{2})", ref):
        raise ValueError("invalid or overlong schema URI")
    urlsplit(ref)
    if ref.startswith("#"):
        return (urldefrag(base)[0] + ref).removesuffix("#")
    return urljoin(base, ref).removesuffix("#")


def compile_embedded(document, path, errors):
    """Validate Draft 7 schema shape and link only to registered schema paths."""
    if errors.full:
        return
    before = len(errors)
    check(draft7_meta, document, path, errors, draft7_meta)
    if len(errors) != before:
        return
    base = REGISTRY_BASE + "__embedded__.json"
    pinned = pinned_schema_positions()
    local = {}
    resources = {base: ()}
    pending = [((), document, base)]
    references = []
    while pending and not errors.full:
        position, node, scope = pending.pop()
        errors.visit()
        local[position] = node
        if isinstance(node, bool):
            continue
        if node.get("$schema", draft7_meta["$id"]).rstrip("#") != draft7_meta["$id"].rstrip("#"):
            errors.append(f"{path}: embedded schema must use Draft 7")
        if "$id" in node:
            scope = schema_uri(scope, node["$id"])
            if scope in pinned or (scope in resources and resources[scope] != position):
                errors.append(f"{path}: duplicate or reserved schema ID {scope!r}")
            else:
                resources[scope] = position
        if "$ref" in node:
            references.append((scope, node["$ref"]))
        for pattern in errors.each(chain((node.get("pattern"),), node.get("patternProperties", {}))):
            if pattern is not None:
                try:
                    if len(pattern.encode("utf-8")) > 2048:
                        raise ValueError("pattern exceeds 2048 bytes")
                    re.compile(pattern)
                except (re.error, RecursionError, ValueError):
                    errors.append(f"{path}: invalid or overlong schema pattern {pattern[:80]!r}")
        definitions = node.get("$defs", {})
        if not isinstance(definitions, dict):
            errors.append(f"{path}: embedded $defs must be an object of schemas")
        for parts, child in errors.each(schema_children(node, errors)):
            # The Draft 7 meta-schema leaves this extension unchecked.
            if parts[0] == "$defs":
                prior = len(errors)
                check(draft7_meta, child, f"{path}.{'.'.join(position + parts)}", errors, draft7_meta)
                if len(errors) != prior or errors.full:
                    continue
            pending.append((position + parts, child, scope))

    for scope, ref in errors.each(references):
        try:
            address = schema_uri(scope, ref)
            document_id, fragment = urldefrag(address)
            if address in resources:
                local[resources[address]]
            elif document_id in pinned:
                pinned[document_id][pointer_tokens(fragment, errors)]
            else:
                local[resources[document_id] + pointer_tokens(fragment, errors)]
        except VisitLimit:
            raise
        except (KeyError, ValueError):
            errors.append(f"{path}: unresolvable embedded schema reference {ref[:256]!r}")


def parse_manifest(text, errors):
    """Validate pinned manifest shape, ID namespaces and schema links before indexing."""
    return validate_manifest(load_strict(text, errors), errors)


def validate_manifest(manifest, errors):
    """Validate the manifest's shape, definitions and install rules in one pass."""
    check(manifest_schema, manifest, "$", errors, manifest_schema)
    if errors:
        return None
    check_unique_ids(
        manifest, errors,
        skip={"schema", "input_schema", "output_schema", "value_schema", "default", "defaults"},
        keys={"$.settings": "key", "$.secrets": "name"},
    )
    for index, operation in enumerate(errors.each(manifest["operations"])):
        if operation["id"].startswith(("host.", "turns.")):
            errors.append(f"$.operations[{index}].id: reserved host operation namespace")
    if errors:
        return None
    for collection, keys in (
        ("kinds", ("schema",)), ("operations", ("input_schema", "output_schema")),
        ("settings", ("value_schema",)), ("credential_routes", ("input_schema", "output_schema")),
    ):
        for index, entry in enumerate(errors.each(manifest.get(collection, []))):
            for key in errors.each(keys):
                if key in entry:
                    compile_embedded(entry[key], f"$.{collection}[{index}].{key}", errors)
    if errors:
        return None
    checked = normalize_manifest(manifest, errors)
    check_manifest_install(checked, errors)
    return None if errors else checked


def load_checked_manifest(path, errors):
    """Read one schema-valid fixture or package manifest, with a path on failure."""
    try:
        manifest = parse_manifest(read_bytes(path).decode("utf-8"), errors)
    except (DuplicateKey, ValueError) as error:
        raise ValueError(f"{path}: {error}") from error
    if errors:
        raise ValueError(f"{path}: {'; '.join(errors)}")
    return manifest


SCHEMA_ANNOTATIONS = {"title", "description", "$comment", "default", "examples", "deprecated", "readOnly", "writeOnly"}


def is_only_item_input(input_schema, field, errors):
    """Require a closed, one-property object with a registered ItemRef, ignoring annotations."""
    if not isinstance(input_schema, dict):
        return False
    properties = input_schema.get("properties")
    shape = {key: value for key, value in errors.each(input_schema.items())
             if key not in SCHEMA_ANNOTATIONS | {"properties", "definitions", "$defs"}}
    if shape != {"type": "object", "additionalProperties": False, "required": [field]}:
        return False
    if not isinstance(properties, dict) or set(properties) != {field}:
        return False
    node, ctx = properties[field], input_schema
    item_ref = CATALOGUE["$defs"]["ItemRef"]
    visited = set()
    while not errors.full and isinstance(node, dict) and id(node) not in visited:
        errors.visit()
        if node is item_ref:
            return True
        visited.add(id(node))
        keywords = set(node) - SCHEMA_ANNOTATIONS
        if keywords != {"$ref"} or not isinstance(node["$ref"], str):
            return False
        node, ctx = resolve(node["$ref"], ctx, errors)
    return False


def composer_send_operations(manifest, errors):
    """Resolve this package's operations reserved for host-only prepared Send."""
    return {kind["composer"]["send"] for kind in errors.each(manifest.get("kinds", {}).values())
            if "composer" in kind}


def check_manifest_install(manifest, errors):
    """Check reserved indexed fields and host_create follow-ups at install."""
    if CATALOGUE is None:
        return
    generic = CATALOGUE["generic_item_fields"]
    projected = CATALOGUE["conversation_capability"]["field_projections"]
    operations = manifest.get("operations", {})
    jobs = manifest.get("jobs", {})
    send_operations = composer_send_operations(manifest, errors)
    composer_jobs = set()
    for kind_id, kind in errors.each(manifest.get("kinds", {}).items()):
        path = f"$.kinds.{kind_id}"
        for field in errors.each(sorted(field_names(kind, errors) & generic.keys())):
            errors.append(f"{path}.indexed_fields.{field}: reserved host generic field")
        if kind.get("conversation") is True:
            for field in errors.each(sorted(field_names(kind, errors) & projected.keys())):
                errors.append(f"{path}.indexed_fields.{field}: reserved host conversation field")
        composer = kind.get("composer")
        if composer:
            send = operations.get(composer.get("send"), {})
            run_job = jobs.get(composer.get("job"), {})
            run = operations.get(run_job.get("run_operation"), {})
            if kind.get("conversation") is not True or send.get("target", {}).get("kind") != kind_id:
                errors.append(f"{path}.composer.send: must name a same-package operation targeting this conversation kind")
            if run_job.get("replayable") is not True or run.get("target", {}).get("kind") != kind_id:
                errors.append(f"{path}.composer.job: must name a replayable job targeting this conversation kind")
            else:
                composer_jobs.add(composer["job"])
                target_field = run["target"].get("field")
                if run_job.get("execution_intent") is not True:
                    errors.append(f"{path}.composer.job: must opt into host execution_intent context")
                if not target_field or not is_only_item_input(run.get("input_schema"), target_field, errors):
                    errors.append(f"{path}.composer.job: run input must accept only its target ItemRef")
        fields = kind.get("fields", {})
        for extractor in errors.each(kind.get("extractors", [])):
            output = fields.get(extractor.get("output"), {})
            allowed = CATALOGUE["query"]["extractors"]["output_types"][extractor["selector"]]
            if kind.get("claimed_text") is not True or output.get("body_derived") is not True:
                errors.append(f"{path}.extractors: requires a claimed-text source and body_derived output")
            if output.get("type") not in allowed:
                errors.append(f"{path}.extractors: {extractor['selector']} cannot produce {output.get('type')!r}")
        for field in errors.each(kind.get("device_row_images", [])):
            if fields.get(field, {}).get("type") != "RowImageRef":
                errors.append(f"{path}.device_row_images: slot must be a declared RowImageRef")
        after = (kind.get("host_create") or {}).get("after")
        if not after:
            continue
        operation_id = after.get("operation")
        if "job" in after:
            if jobs.get(after["job"], {}).get("execution_intent") is True:
                errors.append(f"{path}.host_create.after: a composer intent job can only be admitted by prepared Send")
            operation_id = jobs.get(after["job"], {}).get("run_operation")
        if operation_id in send_operations:
            errors.append(f"{path}.host_create.after: prepared Send is reserved for the host composer")
        operation = operations.get(operation_id, {})
        target = operation.get("target", {})
        field = target.get("field")
        if not field or target.get("kind") != kind_id or not is_only_item_input(operation.get("input_schema"), field, errors):
            errors.append(f"{path}.host_create.after: entry must accept only the created ItemRef of {kind_id!r} at target.field")
    for job_id, job in errors.each(jobs.items()):
        if job.get("execution_intent") is True and job_id not in composer_jobs:
            errors.append(f"$.jobs.{job_id}.execution_intent: must belong to a conversation composer")
    for collection, field in (("jobs", "run_operation"), ("schedule_eligibility", "operation"),
                              ("wait_webhooks", "schema_operation")):
        entries = jobs.values() if collection == "jobs" else manifest.get(collection, [])
        for entry in errors.each(entries):
            if entry[field] in send_operations:
                errors.append(f"$.{collection}.{entry['id']}: prepared Send is reserved for the host composer")


def project_kind(kind, catalogue):
    if kind.get("conversation") is not True:
        return kind
    projected = (catalogue or {}).get("conversation_capability", {}).get("field_projections", {})
    return {**kind, "fields": {**kind.get("fields", {}), **projected}}


def field_meta(kind, field, generic=None):
    if not isinstance(field, str):
        return {}
    if field in (generic or {}):
        return generic[field]
    fields = kind.get("fields")
    meta = fields.get(field) if isinstance(fields, dict) else None
    return meta if isinstance(meta, dict) else {}


def load_fixture_manifest(file, manifests_dir, errors):
    """Read the app fixture's manifest and installed kind owners, when present."""
    manifest_file = manifests_dir / file.name.removeprefix("invalid-")
    if not manifest_file.exists():
        return None, ()
    manifest = load_checked_manifest(manifest_file, errors)
    installed_file = manifests_dir / "installed-for" / f"{manifest.get('id')}.json"
    installed = ()
    if installed_file.exists():
        installed = tuple(
            load_checked_manifest(manifests_dir / owner, errors)
            for owner in errors.each(read_json(installed_file, errors))
        )
    return manifest, installed


def is_reply_fixture(file):
    return file.name.startswith(("reply-", "invalid-reply-"))


def check_reply_origins_shape(origins, reply_files, errors):
    """Validate the whole reply origin sidecar before indexing its references."""
    if not isinstance(origins, dict) or len(origins) > len(reply_files):
        raise ValueError("Reply origins must map descriptions to their reply Binds")
    for description, bound_replies in errors.each(origins.items()):
        if not isinstance(bound_replies, dict) or not 0 < len(bound_replies) <= len(reply_files):
            raise ValueError(f"{description}: expected a map of Bind IDs to reply filenames")
        for bind_id, names in errors.each(bound_replies.items()):
            if not isinstance(names, list) or not 0 < len(names) <= len(reply_files):
                raise ValueError(f"{description}#{bind_id}: expected a list of reply filenames")
            for name in errors.each(names):
                if not isinstance(name, str):
                    raise ValueError(f"{description}#{bind_id}: expected a reply filename string")


def load_reply_origins(origins, source_dir, sources, catalogue, reply_files, errors, *, identify=None):
    """Validate and resolve each reply's description path and Bind to its projection."""
    check_reply_origins_shape(origins, reply_files, errors)
    resolved = {}
    host_kinds = catalogue["kinds"]
    for description, bound_replies in errors.each(origins.items()):
        path = identify(source_dir / description) if identify else resolved_path(source_dir / description, strict=False)
        if path not in sources:
            raise ValueError(f"Unknown reply origin description: {description!r}")
        app, manifest, installed = sources[path]
        binds = {bind["id"]: bind for bind in errors.each(app.get("binds", []))}
        plugin_kinds = {name: kind for owner in errors.each((*installed, manifest)) if owner is not None
                        for name, kind in errors.each(owner.get("kinds", {}).items())}
        for bind_id, names in errors.each(bound_replies.items()):
            if bind_id not in binds:
                raise ValueError(f"{description}: unknown reply origin Bind {bind_id!r}")
            bind = binds[bind_id]
            kind_id = bind["source"]["kind"]
            kind = host_kinds.get(kind_id) or plugin_kinds.get(kind_id)
            if kind is None:
                raise ValueError(f"{description}: unknown kind {kind_id!r} for reply origin Bind {bind_id!r}")
            generic = catalogue["generic_item_fields"] if kind.get("includes_generic_item_fields", kind_id not in host_kinds) else {}
            for name in errors.each(names):
                if name not in reply_files:
                    raise ValueError(f"{description}#{bind_id}: unknown reply filename")
                if name in resolved:
                    raise ValueError(f"{name}: multiple reply origins")
                resolved[name] = bind, project_kind(kind, catalogue), generic
    missing = reply_files - resolved.keys()
    if missing:
        raise ValueError(f"Reply origin coverage mismatch. Missing: {sorted(missing)}.")
    return resolved


def check_window_rows(reply, origin, catalogue, errors):
    """Check each returned value against its declared kind field and nullability."""
    bind, kind, generic = origin
    if "bind" in reply and reply["bind"] != bind["id"]:
        errors.append(f"$.bind: expected origin Bind {bind['id']!r}")
        return
    if "member_cache_record" in reply:
        if bind.get("persist") is not True:
            errors.append(f"$.member_cache_record: origin Bind {bind['id']!r} does not opt in to persistence")
        record = reply["member_cache_record"]
        for field in errors.each(("bind", "window_ref", "offset")):
            if record[field] != reply[field]:
                errors.append(f"$.member_cache_record.{field}: must match its window reply")
    if "rows" not in reply:
        return
    declared_types = catalogue["$defs"]["FieldType"]["enum"]
    for index, row in enumerate(errors.each(reply["rows"])):
        for field in errors.each(row.get("stale_images", [])):
            if (reply["state"] != "ready" or field not in row["fields"] or row["fields"][field] is None
                    or field_meta(kind, field, generic).get("type") != "RowImageRef"):
                errors.append(f"$.rows[{index}].stale_images: must name a delivered RowImageRef in a ready row")
        for field, value in errors.each(row["fields"].items()):
            path = f"$.rows[{index}].fields.{field}"
            meta = field_meta(kind, field, generic)
            field_type = meta.get("type")
            if field_type not in declared_types:
                errors.append(f"{path}: field is not declared with a catalogue type")
            elif field not in bind.get("fields", []):
                errors.append(f"{path}: field is not projected by Bind {bind['id']!r}")
            elif value is not None or meta.get("nullable") is not True:
                check(catalogue["$defs"][field_type], value, path, errors, catalogue)


def validate_reply(doc, origin, errors):
    """Check reply shape, range and projected values against the originating Bind."""
    check({"$ref": "#/$defs/WindowReply"}, doc, "$", errors)
    if errors:
        return
    if doc.get("state") == "ready" and doc.get("count", {}).get("mode") == "exact":
        offset, rows, total = doc["offset"], doc["rows"], doc["count"]["value"]
        if rows and offset + len(rows) > total:
            errors.append("$.count.value: exact count does not cover the returned row range")
        if not rows and offset < total:
            errors.append("$.rows: an exact count still has rows at this offset")
    if origin is not None:
        validate_search_generation(doc, origin[0], errors)
        check_window_rows(doc, origin, CATALOGUE, errors)


def validate_search_generation(value, bind, errors):
    """Require the search correlation field precisely on search Bind windows."""
    if ("settle_generation" in value) != (bind["source"]["type"] == "search"):
        errors.append("$.settle_generation: required for search windows only")
