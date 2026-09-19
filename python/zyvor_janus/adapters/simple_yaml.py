# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Minimal YAML loader for Zynera CRD and profile files (no PyYAML required)."""

from __future__ import annotations

from typing import Any


def safe_load(text: str) -> Any:
    lines = text.splitlines()
    if not any(line.strip() and not line.strip().startswith("#") for line in lines):
        return None
    value, _ = _parse_block(lines, 0, 0)
    return value


def safe_load_all(text: str) -> list[Any]:
    docs: list[Any] = []
    for chunk in text.split("\n---"):
        chunk = chunk.strip()
        if not chunk:
            continue
        doc = safe_load(chunk)
        if doc is not None:
            docs.append(doc)
    return docs


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _parse_scalar(raw: str) -> Any:
    value = raw.strip()
    if not value or value in ("null", "~"):
        return None
    if value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    lower = value.lower()
    if lower == "true":
        return True
    if lower == "false":
        return False
    if value.isdigit() or (value.startswith("-") and value[1:].isdigit()):
        return int(value)
    try:
        if "." in value:
            return float(value)
    except ValueError:
        pass
    return value


def _parse_block(lines: list[str], start: int, base_indent: int) -> tuple[Any, int]:
    i = start
    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.lstrip().startswith("#"):
            i += 1
            continue
        break
    else:
        return None, i

    if lines[i].lstrip().startswith("- "):
        items: list[Any] = []
        while i < len(lines):
            line = lines[i]
            if not line.strip() or line.lstrip().startswith("#"):
                i += 1
                continue
            if _indent(line) < base_indent or not line.lstrip().startswith("- "):
                break
            item_text = line.lstrip()[2:]
            if item_text.strip():
                items.append(_parse_scalar(item_text))
                i += 1
            else:
                i += 1
                child, i = _parse_block(lines, i, base_indent + 2)
                items.append(child)
        return items, i

    mapping: dict[str, Any] = {}
    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.lstrip().startswith("#"):
            i += 1
            continue
        if _indent(line) < base_indent:
            break
        if _indent(line) > base_indent:
            raise ValueError(f"unexpected indent at line {i + 1}: {line!r}")

        key_part, sep, rest = line.strip().partition(":")
        if not sep:
            raise ValueError(f"invalid mapping at line {i + 1}: {line!r}")
        key = key_part.strip()
        i += 1

        if rest.strip():
            mapping[key] = _parse_scalar(rest)
            continue

        if i >= len(lines):
            mapping[key] = None
            continue

        next_line = lines[i]
        if not next_line.strip() or next_line.lstrip().startswith("#"):
            mapping[key] = None
            continue

        child_indent = _indent(next_line)
        if child_indent <= base_indent:
            mapping[key] = None
            continue

        child, i = _parse_block(lines, i, child_indent)
        mapping[key] = child

    return mapping, i
