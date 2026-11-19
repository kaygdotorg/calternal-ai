"""Bounded local input for the plugin v2 verifier."""

import errno
import json
import math
import os
import stat
from collections.abc import Sized
from contextlib import contextmanager
from functools import partial

MAX_ARTIFACT_BYTES = 4 * 1024 * 1024
MAX_PACKAGE_BYTES = 32 * 1024 * 1024
MAX_JSON_DEPTH = 32
MAX_NUMBER_LENGTH = 128
MAX_SAFE_INTEGER = (1 << 53) - 1
MAX_PATH_BYTES = 4096
MAX_PATH_PARTS = 128


class VisitLimit(ValueError):
    """The verification spent its visit budget."""


class Validation:
    """Share a diagnostic cap and visit budget across one verification."""

    MAX_DIAGNOSTICS = 32
    MAX_VISITS = 200_000

    def __init__(self, *, visit_budget=MAX_VISITS):
        self.messages = []
        self.visit_budget = visit_budget
        self.visits = 0

    def __len__(self):
        return len(self.messages)

    def __iter__(self):
        return iter(self.messages)

    @property
    def full(self):
        """Whether another diagnostic would be discarded."""
        return len(self) >= self.MAX_DIAGNOSTICS

    def append(self, message):
        """Record one diagnostic while space remains."""
        if not self.full:
            self.messages.append(message)

    def visit(self, count=1):
        """Charge work before inspecting a node, reference or declaration."""
        self.require_visits(count)
        self.visits += count

    def require_visits(self, count):
        """Refuse a collection whose least possible work exceeds the remaining budget."""
        if self.visits + count > self.visit_budget:
            raise VisitLimit(f"Validation exceeds {self.visit_budget} visits")

    def each(self, items):
        """Charge each yielded element and stop without pulling more at the error cap."""
        if self.full:
            return
        if isinstance(items, Sized):
            self.require_visits(len(items))
        iterator = iter(items)
        while not self.full:
            try:
                item = next(iterator)
            except StopIteration:
                return
            self.visit()
            yield item

    @contextmanager
    def trial(self):
        """Isolate a schema branch's messages while retaining the same budget."""
        saved = self.messages
        self.messages = []
        try:
            yield self.messages
        finally:
            self.messages = saved


class DuplicateKey(ValueError):
    """A JSON object repeated a key."""


def strict_pairs(errors, pairs):
    result = {}
    for key, value in errors.each(pairs):
        if key in result:
            raise DuplicateKey(f"duplicate key {key!r}")
        result[key] = value
    return result


def strict_int(token):
    if len(token) <= MAX_NUMBER_LENGTH:
        value = int(token)
        if abs(value) <= MAX_SAFE_INTEGER:
            return value
    raise ValueError("JSON integer outside the portable safe range")


def strict_float(token):
    if len(token) <= MAX_NUMBER_LENGTH:
        value = float(token)
        if math.isfinite(value):
            return value
    raise ValueError("JSON number outside the finite binary64 range")


def reject_json_constant(token):
    raise ValueError(f"non-JSON constant {token}")


def check_json_depth(text):
    # Lexical scanning is bounded by the artifact byte limit. Collection visits
    # are accounted for when JSON object pairs and values are inspected.
    depth = 0
    quoted = escaped = False
    for char in text:
        if quoted:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
        elif char in "{[":
            depth += 1
            if depth > MAX_JSON_DEPTH:
                raise ValueError(f"JSON exceeds {MAX_JSON_DEPTH} nested containers")
        elif char in "}]":
            depth -= 1


def load_strict(text, errors=None):
    """Parse bounded JSON without duplicate keys, unsafe integers or nonfinite values."""
    if errors is None:
        errors = Validation()
    check_json_depth(text)
    value = json.loads(text, object_pairs_hook=partial(strict_pairs, errors), parse_int=strict_int,
                       parse_float=strict_float, parse_constant=reject_json_constant)
    validate_json_strings(value, errors)
    return value


def validate_json_strings(value, errors):
    if isinstance(value, dict):
        for key, child in errors.each(value.items()):
            if len(key.encode("utf-8")) > 256:
                raise ValueError("JSON object key exceeds 256 UTF-8 bytes")
            validate_json_strings(child, errors)
    elif isinstance(value, list):
        for child in errors.each(value):
            validate_json_strings(child, errors)
    elif isinstance(value, str):
        value.encode("utf-8")  # Reject unpaired surrogates, including in JSON escapes.


def check_path_length(path):
    if len(os.fsencode(path)) > MAX_PATH_BYTES or len(path.parts) > MAX_PATH_PARTS:
        raise ValueError("Artifact path exceeds 4096 bytes or 128 components")


def resolved_path(path, *, strict=True):
    """Resolve a bounded filesystem path, including an optional logical template alias."""
    check_path_length(path)
    try:
        candidate = path.resolve(strict=strict)
    except RuntimeError as error:
        raise ValueError(f"{path}: cannot resolve artifact path") from error
    check_path_length(candidate)
    return candidate


DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
READ_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK


def lexical_path(path):
    """Bound a path without consulting the filesystem."""
    check_path_length(path)
    path = path.absolute()
    check_path_length(path)
    if ".." in path.parts:
        raise ValueError(f"{path}: parent path components are not permitted")
    return path


def descend_directory(directory, parts, errors):
    """Open each directory component relative to a trusted descriptor."""
    if errors.full:
        raise ValueError("Cannot open an artifact after the diagnostic cap")
    current = os.dup(directory)
    try:
        for part in errors.each(parts):
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=current)
            os.close(current)
            current = child
        return current
    except BaseException:
        os.close(current)
        raise


class PackageRoot:
    """Keep the original package directory open for every contained lookup and read."""

    def __init__(self, path, errors):
        self.path = lexical_path(path)
        self.errors = errors
        self.fd = None

    def __enter__(self):
        start = os.open(self.path.anchor, DIRECTORY_FLAGS)
        try:
            self.fd = descend_directory(start, self.path.parts[1:], self.errors)
        except OSError as error:
            raise ValueError(f"{self.path}: cannot open package root without symlinks: {error}") from error
        finally:
            os.close(start)
        return self

    def __exit__(self, *_):
        os.close(self.fd)
        self.fd = None

    def path_for(self, path):
        candidate = lexical_path(path)
        if not candidate.is_relative_to(self.path):
            raise ValueError(f"{path}: artifact leaves the supplied package directory")
        return candidate

    def open(self, path, flags):
        parts = self.path_for(path).relative_to(self.path).parts
        directory = descend_directory(self.fd, parts[:-1], self.errors)
        if not parts:
            return directory
        try:
            return os.open(parts[-1], flags, dir_fd=directory)
        except OSError as error:
            if error.errno in (errno.ELOOP, errno.ENOTDIR):
                raise ValueError(f"{path}: symlink or invalid artifact path") from error
            raise
        finally:
            os.close(directory)


def read_fd(fd, path, limit):
    """Read no more than the byte budget from a regular file descriptor."""
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode):
            raise ValueError(f"{path}: artifact must be a regular file")
        if info.st_size > limit:
            raise ValueError(f"{path}: artifact exceeds its {limit}-byte budget")
    except BaseException:
        os.close(fd)
        raise
    with os.fdopen(fd, "rb") as stream:
        data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError(f"{path}: artifact exceeds its {limit}-byte budget")
        return data


def read_bytes(path):
    """Read a trusted local or explicitly selected regular file."""
    resolved = resolved_path(path)
    return read_fd(os.open(resolved, READ_FLAGS), resolved, MAX_ARTIFACT_BYTES)


def read_json(path, errors=None):
    """Read bounded UTF-8 JSON from an explicitly selected path."""
    return load_strict(read_bytes(path).decode("utf-8"), errors)


class PackageInput:
    """Share the package byte budget and a retained root across artifact reads."""

    def __init__(self, boundary, errors=None):
        self.errors = Validation() if errors is None else errors
        self.root = PackageRoot(boundary, self.errors) if boundary is not None else None
        self.remaining = MAX_PACKAGE_BYTES

    def __enter__(self):
        if self.root:
            self.root.__enter__()
        return self

    def __exit__(self, *exc):
        if self.root:
            self.root.__exit__(*exc)

    def identify(self, path):
        """Identify a template alias without opening it."""
        return self.root.path_for(path) if self.root else resolved_path(path, strict=False)

    def bytes(self, path, *, implicit_root=None):
        """Open an artifact once under the applicable path and byte restrictions."""
        if self.errors.full:
            raise ValueError("Cannot read an artifact after the diagnostic cap")
        self.errors.visit()
        limit = min(self.remaining, MAX_ARTIFACT_BYTES)
        if self.root:
            resolved = self.root.path_for(path)
            data = read_fd(self.root.open(resolved, READ_FLAGS), resolved, limit)
        elif implicit_root:
            with PackageRoot(implicit_root, self.errors) as root:
                resolved = root.path_for(path)
                data = read_fd(root.open(resolved, READ_FLAGS), resolved, limit)
        else:
            resolved = resolved_path(path)
            data = read_fd(os.open(resolved, READ_FLAGS), resolved, limit)
        self.remaining -= len(data)
        return resolved, data

    def json(self, path):
        """Parse one byte-budgeted JSON artifact."""
        _, data = self.bytes(path)
        return load_strict(data.decode("utf-8"), self.errors)

    def reply_files(self, path):
        """Discover a bounded flat reply directory using the retained root."""
        try:
            fd = self.root.open(path, DIRECTORY_FLAGS)
        except FileNotFoundError:
            return []
        try:
            with os.scandir(fd) as entries:
                result = []
                for index, entry in enumerate(self.errors.each(entries)):
                    if index >= 128:
                        raise ValueError("Reply directory exceeds 128 entries")
                    if entry.name.endswith(".json") and entry.name != "origins.json":
                        result.append(path / entry.name)
                return sorted(result)
        finally:
            os.close(fd)


def check_unique_ids(value, errors, path="$", *, skip=frozenset(), keys=None):
    """Check each declaration list's identity key before building indexes."""
    if errors.full:
        return
    errors.visit()
    if isinstance(value, list):
        key = (keys or {}).get(path, "id")
        seen = set()
        for index, entry in enumerate(errors.each(value)):
            location = f"{path}[{index}]"
            ident = entry.get(key) if isinstance(entry, dict) else None
            if isinstance(ident, str):
                if ident in seen:
                    errors.append(f"{location}.{key}: duplicate {key} {ident!r} in {path}")
                seen.add(ident)
            check_unique_ids(entry, errors, location, skip=skip, keys=keys)
    elif isinstance(value, dict):
        for key, entry in errors.each(value.items()):
            if key not in skip:
                check_unique_ids(entry, errors, f"{path}.{key}", skip=skip, keys=keys)
