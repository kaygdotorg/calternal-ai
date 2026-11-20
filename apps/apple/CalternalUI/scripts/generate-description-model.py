#!/usr/bin/env python3
"""Generates the Swift model of the v2 description contract from the sealed schemas.

    python3 scripts/generate-description-model.py            # writes Sources/CalternalUI/Description/Generated/
    python3 scripts/generate-description-model.py --check    # exits 1 when the generated files are out of date

Inputs (never copied by hand): `contract/description-v2.schema.json` (the description shapes) and `contract/host-catalogue-v2.json`
(`$defs`: the wire shapes of the host). Output: `V2.<Def>` for the description schema and `V2.Cat.<Def>` for the catalogue, one Swift type per
`$defs` entry, plus one per inline object or union (named after its parent).

What is generated, what is not:
* structure is strict: an unknown key on an object whose schema says `additionalProperties: false`, a missing required key and a wrong JSON type all throw
  `V2Error`; a `oneOf` tries its branches in order (the strict keys make object branches unambiguous);
* value constraints (patterns, lengths, item counts) and the conditional `if`/`then`/`allOf` rules are NOT checked here: they are the verifier's job
  (`contract/verify-description.py`), and the host is the authority. The model is what a renderer needs to draw, not a second validator;
* every generated type has `init(json:path:)` and `var json` (the exact inverse), so a fixture decoded and re-encoded must equal itself: that is the test that
  nothing in the contract is silently dropped.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parents[1] / "Sources" / "CalternalUI" / "Description" / "Generated"
SWIFT_KEYWORDS = {
    "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func", "import", "init", "inout", "internal", "let", "open", "operator", "private",
    "precedencegroup", "protocol", "public", "rethrows", "static", "struct", "subscript", "typealias", "var", "break", "case", "catch", "continue", "default",
    "defer", "do", "else", "fallthrough", "for", "guard", "if", "in", "repeat", "return", "throw", "switch", "where", "while", "Any", "as", "false", "is", "nil",
    "self", "Self", "super", "throws", "true", "try", "Type", "Protocol",
}


def lower_camel(name: str) -> str:
    parts = re.split(r"[^A-Za-z0-9]+", name)
    parts = [p for p in parts if p]
    if not parts:
        return "value"
    head = parts[0][0].lower() + parts[0][1:]
    out = head + "".join(p[0].upper() + p[1:] for p in parts[1:])
    if out[0].isdigit():
        out = "v" + out
    return f"`{out}`" if out in SWIFT_KEYWORDS else out


def upper_camel(name: str) -> str:
    parts = [p for p in re.split(r"[^A-Za-z0-9]+", name) if p]
    out = "".join(p[0].upper() + p[1:] for p in parts) or "Value"
    return "V" + out if out[0].isdigit() else out


class Gen:
    def __init__(self, schema: dict, namespace: str, cat_namespace: str | None, other: dict | None):
        self.schema = schema
        self.ns = namespace  # "" for the description schema, "Cat." for the catalogue
        self.cat = cat_namespace
        self.defs = schema.get("$defs", {})
        self.other_defs = (other or {}).get("$defs", {})
        self.out: list[str] = []
        self.emitted: set[str] = set()
        self.names: dict[str, str] = {}

    # ---- type resolution -------------------------------------------------------------------------------------------------------------------------

    def ref_target(self, ref: str):
        """Returns (defs-owner, name) for a $ref."""
        if ref.startswith("#/$defs/"):
            return self, ref.split("/")[-1]
        if "host-catalogue-v2.json#/$defs/" in ref:
            return "cat", ref.split("/")[-1]
        raise SystemExit(f"unsupported $ref {ref}")

    def is_primitive(self, node: dict):
        """A plain primitive (no enum, no union): mapped straight to a Swift primitive."""
        if "oneOf" in node or "enum" in node or "const" in node:
            return None
        t = node.get("type")
        if isinstance(t, list):
            return "scalar"
        return {"string": "String", "integer": "Int", "number": "Double", "boolean": "Bool"}.get(t)

    def swift_type(self, node: dict, hint: str) -> str:
        """The Swift type of a schema node; emits helper types for inline objects, enums and unions."""
        if isinstance(node, bool):
            return "JSONValue"
        if "$ref" in node:
            owner, name = self.ref_target(node["$ref"])
            if owner == "cat":
                target = self.other_defs[name]
                prim = self.is_primitive(target)
                if prim:
                    return "JSONValue" if prim == "scalar" else prim
                if not target.get("properties") and (target.get("type") in ("object", "array") or ("const" in target and "type" not in target)):
                    return self.define_inline(target, name)
                return f"Cat.{upper_camel(name)}"
            target = self.defs[name]
            prim = self.is_primitive(target)
            if prim:
                return "JSONValue" if prim == "scalar" else prim
            if not target.get("properties") and (target.get("type") in ("object", "array") or ("const" in target and "type" not in target)):
                return self.define_inline(target, name)
            if "$ref" in target and len(target) <= 2:
                return self.swift_type(target, name)
            return upper_camel(name)
        return self.define_inline(node, hint)

    def define_inline(self, node: dict, hint: str) -> str:
        prim = self.is_primitive(node)
        if prim:
            return "JSONValue" if prim == "scalar" else prim
        if "const" in node:
            return {str: "String", bool: "Bool", int: "Int", float: "Double"}.get(type(node["const"]), "JSONValue")
        if "enum" in node:
            name = upper_camel(hint)
            self.emit_enum(name, node["enum"])
            return name
        if "oneOf" in node:
            name = upper_camel(hint)
            self.emit_union(name, node)
            return name
        t = node.get("type")
        if t == "array":
            return "[" + self.swift_type(node.get("items", {}), hint + "Item") + "]"
        if t == "object" or "properties" in node:
            props = node.get("properties")
            if not props:
                ap = node.get("additionalProperties")
                if isinstance(ap, dict):
                    return "[String: " + self.swift_type(ap, hint + "Value") + "]"
                return "[String: JSONValue]"
            name = upper_camel(hint)
            self.emit_struct(name, node)
            return name
        return "JSONValue"

    # ---- decode and encode expressions ------------------------------------------------------------------------------------------------------------

    def decode(self, swift: str, expr: str, path: str) -> str:
        if swift == "JSONValue":
            return expr
        if swift in ("String", "Int", "Double", "Bool"):
            return f"try {expr}.as{swift}({path})"
        if swift.startswith("[String: "):
            inner = swift[len("[String: "):-1]
            return f"try {expr}.asObject({path}).mapValues {{ {self.decode(inner, '$0', path)} }}"
        if swift.startswith("["):
            inner = swift[1:-1]
            return f"try {expr}.asArray({path}).enumerated().map {{ {self.decode(inner, '$1', 'path + \"[\\($0)]\"')} }}"
        return f"try {swift}(json: {expr}, path: {path})"

    def encode(self, swift: str, expr: str) -> str:
        if swift == "JSONValue":
            return expr
        if swift == "String":
            return f".string({expr})"
        if swift == "Int":
            return f".number(Double({expr}))"
        if swift == "Double":
            return f".number({expr})"
        if swift == "Bool":
            return f".bool({expr})"
        if swift.startswith("[String: "):
            inner = swift[len("[String: "):-1]
            return f".object({expr}.mapValues {{ {self.encode(inner, '$0')} }})"
        if swift.startswith("["):
            inner = swift[1:-1]
            return f".array({expr}.map {{ {self.encode(inner, '$0')} }})"
        return f"{expr}.json"

    # ---- emitters -----------------------------------------------------------------------------------------------------------------------------------

    def emit_enum(self, name: str, values: list):
        if name in self.emitted:
            return
        self.emitted.add(name)
        lines = [f"    public enum {name}: String, Equatable, Hashable, Sendable, CaseIterable {{"]
        seen = set()
        for v in values:
            case = lower_camel(str(v)) if str(v) else "empty"
            while case in seen:
                case += "_"
            seen.add(case)
            lines.append(f"        case {case} = {json.dumps(str(v))}")
        lines.append("        init(json: JSONValue, path: String) throws {")
        lines.append("            let raw = try json.asString(path)")
        lines.append("            guard let value = Self(rawValue: raw) else { throw V2Error.invalid(path, \"\\(raw) is not one of the declared values\") }")
        lines.append("            self = value")
        lines.append("        }")
        lines.append("        var json: JSONValue { .string(rawValue) }")
        lines.append("    }")
        self.out.append("\n".join(lines))

    def emit_struct(self, name: str, node: dict):
        if name in self.emitted:
            return
        self.emitted.add(name)
        props: dict = node.get("properties", {})
        required = set(node.get("required", []))
        strict = node.get("additionalProperties") is False
        fields = []
        for key, sub in props.items():
            swift = self.swift_type(sub, name + upper_camel(key))
            optional = key not in required
            const = sub.get("const") if isinstance(sub, dict) else None
            fields.append((key, lower_camel(key), swift, optional, const, sub))
        desc = (node.get("description") or "").replace("\n", " ")
        lines = []
        if desc:
            lines.append(f"    /// {desc[:240]}")
        lines.append(f"    public struct {name}: Equatable, Sendable {{")
        for key, prop, swift, optional, const, sub in fields:
            lines.append(f"        public var {prop}: {swift}{'?' if optional else ''}")
        init_args = ", ".join(f"{prop}: {swift}{'?' if optional else ''}{' = nil' if optional else ''}" for key, prop, swift, optional, const, sub in fields)
        lines.append(f"        public init({init_args}) {{")
        for key, prop, swift, optional, const, sub in fields:
            lines.append(f"            self.{prop} = {prop}")
        lines.append("        }")
        lines.append("        init(json: JSONValue, path: String) throws {")
        lines.append("            let o = try json.asObject(path)")
        allowed = ", ".join(json.dumps(k) for k in props)
        if strict:
            lines.append(f"            try V2.checkKeys(o, allowed: [{allowed}], path: path)")
        for key, prop, swift, optional, const, sub in fields:
            expr = f"o[{json.dumps(key)}]"
            keypath = f"path + \".{key}\""
            target = prop
            if optional:
                lines.append(f"            if let v = {expr}, v != .null {{ self.{target} = {self.decode(swift, 'v', keypath)} }} else {{ self.{target} = nil }}")
            else:
                lines.append(f"            guard let {prop.strip('`')}Raw = {expr} else {{ throw V2Error.missing({keypath}) }}")
                lines.append(f"            self.{target} = {self.decode(swift, prop.strip('`') + 'Raw', keypath)}")
            if const is not None:
                lit = json.dumps(const) if isinstance(const, str) else (str(const).lower() if isinstance(const, bool) else str(const))
                if optional:
                    lines.append(f"            if let c = self.{target}, c != {lit} {{ throw V2Error.invalid({keypath}, \"must be {json.dumps(const)[1:-1]}\") }}")
                else:
                    lines.append(f"            if self.{target} != {lit} {{ throw V2Error.invalid({keypath}, \"must be {json.dumps(const)[1:-1]}\") }}")
        lines.append("        }")
        lines.append("        var json: JSONValue {")
        lines.append("            var o: [String: JSONValue] = [:]")
        for key, prop, swift, optional, const, sub in fields:
            enc = self.encode(swift, "v")
            if optional:
                lines.append(f"            if let v = self.{prop} {{ o[{json.dumps(key)}] = {enc} }}")
            else:
                lines.append(f"            do {{ let v = self.{prop}; o[{json.dumps(key)}] = {enc} }}")
        lines.append("            return .object(o)")
        lines.append("        }")
        lines.append("    }")
        self.out.append("\n".join(lines))

    def emit_union(self, name: str, node: dict):
        if name in self.emitted:
            return
        self.emitted.add(name)
        branches = node["oneOf"]
        cases = []
        used = set()
        for i, br in enumerate(branches):
            if "$ref" in br:
                owner, tname = self.ref_target(br["$ref"])
                case = lower_camel(tname)
                swift = self.swift_type(br, name + upper_camel(tname))
            else:
                if "properties" in br:
                    req = br.get("required") or list(br["properties"])
                    case = lower_camel("_".join(req[:2]))
                elif "const" in br:
                    case = lower_camel("const_" + str(br["const"]))
                elif br.get("type") in ("string", "number", "integer", "boolean", "null", "array"):
                    case = br["type"] if br["type"] != "boolean" else "bool"
                    if br["type"] == "array":
                        case = "list"
                else:
                    case = f"variant{i}"
                swift = "JSONValue" if br.get("type") == "null" else self.swift_type(br, name + upper_camel(case) + ("Case" if case in used else ""))
            while case in used:
                case += "_"
            used.add(case)
            cases.append((case, swift, br))
        desc = (node.get("description") or "").replace("\n", " ")
        lines = []
        if desc:
            lines.append(f"    /// {desc[:240]}")
        lines.append(f"    public indirect enum {name}: Equatable, Sendable {{")
        for case, swift, br in cases:
            if br.get("type") == "null":
                lines.append(f"        case {case}")
            else:
                lines.append(f"        case {case}({swift})")
        lines.append("        init(json: JSONValue, path: String) throws {")
        for case, swift, br in cases:
            if br.get("type") == "null":
                lines.append(f"            if case .null = json {{ self = .{case}; return }}")
            elif "const" in br:
                lines.append(f"            if let v = {self.decode(swift, 'json', 'path').replace('try ', 'try? ', 1)}, v == {json.dumps(br['const']) if isinstance(br['const'], str) else str(br['const']).lower()} {{ self = .{case}(v); return }}")
            else:
                lines.append(f"            if let v = {self.decode(swift, 'json', 'path').replace('try ', 'try? ', 1)} {{ self = .{case}(v); return }}")
        lines.append(f"            throw V2Error.invalid(path, \"matches no variant of {name}\")")
        lines.append("        }")
        lines.append("        var json: JSONValue {")
        lines.append("            switch self {")
        for case, swift, br in cases:
            if br.get("type") == "null":
                lines.append(f"            case .{case}: return .null")
            else:
                lines.append(f"            case .{case}(let v): return {self.encode(swift, 'v')}")
        lines.append("            }")
        lines.append("        }")
        lines.append("    }")
        self.out.append("\n".join(lines))

    def emit_def(self, name: str, node: dict):
        tname = upper_camel(name)
        if self.is_primitive(node):
            return
        if "$ref" in node and len(node) <= 2:
            return
        if "not" in node and "type" not in node:
            return   # a constraint only (it forbids a shape), not a type: the verifier checks it
        if "oneOf" in node and node["oneOf"] and all(isinstance(m, dict) and set(m) == {"required"} for m in node["oneOf"]):
            node = {k: v for k, v in node.items() if k != "oneOf"}   # "exactly one of these properties": the struct has them all optional
        if "enum" in node:
            self.emit_enum(tname, node["enum"])
        elif "oneOf" in node:
            self.emit_union(tname, node)
        elif node.get("type") == "object" or "properties" in node:
            if not node.get("properties"):
                ap = node.get("additionalProperties")
                inner = self.swift_type(ap, tname + "Value") if isinstance(ap, dict) else "JSONValue"
                self.out.append(f"    public typealias {tname} = [String: {inner}]")
                self.emitted.add(tname)
                return
            self.emit_struct(tname, node)
        elif node.get("type") == "array":
            inner = self.swift_type(node.get("items", {}), tname + "Item")
            self.out.append(f"    public typealias {tname} = [{inner}]")
            self.emitted.add(tname)
        elif "const" in node:
            self.out.append(f"    public typealias {tname} = String")
            self.emitted.add(tname)
        else:
            self.out.append(f"    public typealias {tname} = JSONValue")
            self.emitted.add(tname)

    def generate(self) -> str:
        for name, node in self.defs.items():
            self.emit_def(name, node)
        return "\n\n".join(self.out)


def anyof_as_oneof(node):
    """A union is a union: `anyOf` branches are tried in order, like `oneOf` (the verifier owns the difference).
    `{"allOf": [{"$ref": T}, ...constraints]}` is the type `T` (the further members only constrain it; the verifier checks them)."""
    if isinstance(node, dict):
        al = node.get("allOf")
        if isinstance(al, list) and al and isinstance(al[0], dict) and "$ref" in al[0] and len(node) == 1:
            node.clear(); node.update(al[0])
        if "anyOf" in node and "oneOf" not in node:
            node["oneOf"] = node.pop("anyOf")
        for v in node.values():
            anyof_as_oneof(v)
    elif isinstance(node, list):
        for v in node:
            anyof_as_oneof(v)
    return node


def main():
    desc = anyof_as_oneof(json.loads((ROOT / "contract" / "description-v2.schema.json").read_text()))
    cat = anyof_as_oneof(json.loads((ROOT / "contract" / "host-catalogue-v2.json").read_text()))
    header = ("// Generated by scripts/generate-description-model.py from contract/description-v2.schema.json and contract/host-catalogue-v2.json. Do not edit.\n"
              "// Structure is strict; value constraints and conditional rules stay with the verifier (contract/verify-description.py).\n\n")
    cat_gen = Gen(cat, "Cat.", None, desc)
    cat_text = cat_gen.generate()
    desc_gen = Gen(desc, "", "Cat.", cat)
    desc_text = desc_gen.generate()
    files = {
        "CatalogueModel.swift": header + "extension V2 {\n    public enum Cat {}\n}\n\nextension V2.Cat {\n" + re.sub(r"(?m)^", "", cat_text).replace("\n    ", "\n    ") + "\n}\n",
        "DescriptionModel.swift": header + "extension V2 {\n" + desc_text + "\n}\n",
    }
    check = "--check" in sys.argv
    OUT.mkdir(parents=True, exist_ok=True)
    stale = False
    for name, text in files.items():
        # catalogue types are referenced inside V2 as Cat.X: they live in V2.Cat, so inside it they refer to each other without the prefix
        if name == "CatalogueModel.swift":
            pass
        path = OUT / name
        if check:
            if not path.exists() or path.read_text() != text:
                print(f"stale: {path}")
                stale = True
        else:
            path.write_text(text)
            print(f"wrote {path} ({len(text.splitlines())} lines)")
    sys.exit(1 if stale else 0)


if __name__ == "__main__":
    main()
