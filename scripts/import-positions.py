#!/usr/bin/env python3
"""Import test positions from a dfa-games commit into config/.

    scripts/import-positions.py DFA_GAMES_REPO COMMIT [GAME ...]

Reads config/<game>/positions-manual.json and positions-generated.json as of
COMMIT (through git, never the working tree) and splits every case by what it
checks:

    config/<game>/moves.json    expected_moves
    config/<game>/perft_u.json  expected_perft_u
    config/<game>/results.json  expected_result

A case carrying several checks lands in several files.  Imported cases get a
"source" naming the commit.

Cases are keyed by position and side to move within each file.  A new key is
added, an identical check is left alone, and a different check under an
existing key is a conflict: the import stops and writes nothing, so positions
written here are never overwritten.  Re-running an import is a no-op.
"""

import argparse
import json
import pathlib
import subprocess
import sys

KINDS = {
    "moves": "expected_moves",
    "perft_u": "expected_perft_u",
    "results": "expected_result",
}
SOURCE_FILES = ["positions-manual.json", "positions-generated.json"]
KNOWN_KEYS = {"comment", "position", "side_to_move"} | set(KINDS.values())

CONFIG = pathlib.Path(__file__).resolve().parent.parent / "config"


class ImportError_(Exception):
    pass


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args], check=True, capture_output=True, text=True
    ).stdout


def comparable(kind, value):
    """moves are a set; everything else compares as is."""
    if kind == "moves":
        return sorted(value)
    return value


def key_of(case):
    return (tuple(case["position"]), case["side_to_move"])


def dumps(value, indent=0):
    """JSON with two space indent and flat number arrays on one line."""
    pad = "  " * indent
    inner = "  " * (indent + 1)
    if isinstance(value, dict):
        if not value:
            return "{}"
        items = [
            f"{inner}{json.dumps(k)}: {dumps(v, indent + 1)}" for k, v in value.items()
        ]
        return "{\n" + ",\n".join(items) + "\n" + pad + "}"
    if isinstance(value, list):
        if all(not isinstance(v, (dict, list)) for v in value):
            return "[" + ", ".join(json.dumps(v) for v in value) + "]"
        items = [inner + dumps(v, indent + 1) for v in value]
        return "[\n" + ",\n".join(items) + "\n" + pad + "]"
    return json.dumps(value)


def split_case(case, where):
    unknown = set(case) - KNOWN_KEYS
    if unknown:
        raise ImportError_(f"{where} has unknown keys {sorted(unknown)}")
    for required in ("position", "side_to_move"):
        if required not in case:
            raise ImportError_(f"{where} has no {required!r}")

    output = []
    for kind, field in KINDS.items():
        if field not in case:
            continue
        piece = {}
        if "comment" in case:
            piece["comment"] = case["comment"]
        piece["position"] = case["position"]
        piece["side_to_move"] = case["side_to_move"]
        piece[field] = case[field]
        output.append((kind, piece))

    if not output:
        raise ImportError_(f"{where} checks nothing")
    return output


def load_existing(game):
    files = {}
    for kind in KINDS:
        path = CONFIG / game / f"{kind}.json"
        if path.exists():
            data = json.loads(path.read_text())
            if data.get("game") != game:
                raise ImportError_(f"{path} is for {data.get('game')!r}, not {game!r}")
        else:
            data = {"game": game, "positions": []}
        files[kind] = data
    return files


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("repo", help="dfa-games checkout (only its git history is read)")
    parser.add_argument("commit", help="dfa-games commit to import from")
    parser.add_argument("games", nargs="*", help="games to import (default: all)")
    args = parser.parse_args()

    sha = git(args.repo, "rev-parse", "--verify", f"{args.commit}^{{commit}}").strip()
    source = f"dfa-games {sha}"

    available = {}
    for path in git(args.repo, "ls-tree", "-r", "--name-only", sha, "config").split():
        parts = path.split("/")
        if len(parts) == 3 and parts[2] in SOURCE_FILES:
            available.setdefault(parts[1], []).append(parts[2])

    games = args.games or sorted(available)
    missing = [g for g in games if g not in available]
    if missing:
        raise ImportError_(f"no positions for {missing} at {sha}")

    conflicts = []
    changed = {}
    added = {kind: 0 for kind in KINDS}
    unchanged = {kind: 0 for kind in KINDS}

    for game in games:
        files = load_existing(game)
        index = {
            kind: {key_of(c): c for c in data["positions"]} for kind, data in files.items()
        }

        for filename in SOURCE_FILES:
            if filename not in available[game]:
                continue
            path = f"config/{game}/{filename}"
            data = json.loads(git(args.repo, "show", f"{sha}:{path}"))
            if data.get("game") != game:
                raise ImportError_(f"{path} is for {data.get('game')!r}, not {game!r}")

            for i, case in enumerate(data["positions"]):
                where = f"{path} case {i}"
                for kind, piece in split_case(case, where):
                    field = KINDS[kind]
                    key = key_of(piece)
                    existing = index[kind].get(key)
                    if existing is None:
                        piece["source"] = source
                        files[kind]["positions"].append(piece)
                        index[kind][key] = piece
                        changed[(game, kind)] = files[kind]
                        added[kind] += 1
                    elif comparable(kind, existing[field]) == comparable(kind, piece[field]):
                        unchanged[kind] += 1
                    else:
                        conflicts.append(
                            f"{where}: {field} {piece[field]!r} conflicts with "
                            f"config/{game}/{kind}.json {existing[field]!r}"
                        )

    if conflicts:
        raise ImportError_("conflicts, nothing written:\n  " + "\n  ".join(conflicts))

    for (game, kind), data in sorted(changed.items()):
        path = CONFIG / game / f"{kind}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(dumps(data) + "\n")

    for kind in KINDS:
        print(f"{kind}: {added[kind]} added, {unchanged[kind]} already present")
    print(f"{len(changed)} files written from {source}, {len(games)} games")


if __name__ == "__main__":
    try:
        main()
    except (ImportError_, subprocess.CalledProcessError) as e:
        detail = getattr(e, "stderr", None)
        print(f"import-positions: {e}{': ' + detail.strip() if detail else ''}", file=sys.stderr)
        sys.exit(1)
