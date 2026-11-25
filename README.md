# dfa-verify

An independent verifier for the solver output of [dfa-games](https://github.com/jconsidi/dfa-games).

dfa-games solves combinatorial games by representing sets of positions as layered DFAs.
Its solver is C++ and nobody checks its answers by hand.
This repository reads the DFAs it writes and checks them against game rules implemented separately in Rust, so that a wrong result has to be wrong in two independent implementations to go unnoticed.
Nothing here constructs a DFA.

## Layout

- `dfa-format/` — reader, writer and validator for the `.dfa` file format described in `FORMAT-DFA.md`.
  Binaries: `dfa-validate`, `dfa-stats`, `dfa-verify-union`.
- `dfa-games/` — position level rules for each game and the verifiers over them.
  Binaries: `verify-lost-sound`, `verify-won-sound`, `verify-losing-sound`, `verify-winning-sound`, `verify-backward-sound`, taking the same arguments as their C++ namesakes in dfa-games.
- `GAMES.md` — the rules of each game, written from the games themselves.
- `FORMAT-DFA.md` — the `.dfa` file format.
- `config/<game>/{moves,perft_u,results}.json` — test positions for the rules (see **Testing**).
- `scripts/import-positions.py` — imports test positions from dfa-games (see **Provenance**).
- `TODO.md` — known gaps.

## Building

    cargo build --release

## Running

Every binary reads DFAs from a dfa-games scratch directory, holding `<game>/` and `dfas_by_hash/`.
Point at it with `--scratch` or `DFA_ARCHIVE_DIR`; the default is `./scratch`.

    target/release/verify-backward-sound --scratch ~/projects/dfa-games/src/scratch breakthrough_4x4 1

## Testing

    cargo test --release

Besides unit and format tests, this checks the rules against `config/<game>/`, one file per kind of check:

- `moves.json` — `expected_moves`, every position one move away.
- `perft_u.json` — `expected_perft_u`, the number of distinct positions at each depth.
- `results.json` — `expected_result`, the result at a terminal position for the side to move (-1, 0 or 1), or null for a position that is not terminal.
  This is the game's end condition, not its value under optimal play.

Each case also has `position`, `side_to_move`, and optionally `comment` and `source`.
The schema is strict: an unknown field, a missing one, a position listed twice in a file, an unrecognized file under `config/`, or a game the Rust cannot build fails the run rather than being skipped.

## Provenance

This repository was split from dfa-games, where it lived under `rust/`, with its history filtered to that directory plus `GAMES.md` and `FORMAT-DFA.md`.
The split point is dfa-games commit `ef1bcd7c23d47a979f5856e998a236d0b9cf8e2b`.

Test positions are imported from dfa-games rather than carried over with history, since most of the history of dfa-games `config/` concerns the C++ solver's symbolic move generation.

    scripts/import-positions.py ~/projects/dfa-games <commit> [game ...]

reads dfa-games `config/<game>/positions-manual.json` and `positions-generated.json` at that commit through git and splits each case into the files above, marking imported cases with `"source": "dfa-games <sha>"`.
Cases are matched by position and side to move: new ones are added, identical ones are left alone, and a conflicting one stops the import before anything is written, so positions added here are never overwritten.
The initial import was from the split commit.
