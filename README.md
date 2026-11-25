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
- `config/<game>/positions-manual.json`, `config/<game>/positions-generated.json` — test positions for the rules (see **Testing**).
- `TODO.md` — known gaps.

## Building

    cargo build --release

## Running

Every binary reads DFAs from a dfa-games scratch directory, holding `<game>/` and `dfas_by_hash/`.
Point at it with `--scratch` or `DFA_ARCHIVE_DIR`; the default is `./scratch`.

    target/release/verify-backward-sound --scratch ~/projects/dfa-games/src/scratch breakthrough_4x4 1

## Testing

    cargo test --release

Besides unit and format tests, this checks the rules against `config/`:
`positions-manual.json` holds hand written positions with their expected moves and results, and `positions-generated.json` holds `expected_perft_u` counts per configured size.
A file naming a game the Rust cannot build fails the run rather than being skipped.

## Provenance

This repository was split from dfa-games, where it lived under `rust/`, with its history filtered to that directory plus `GAMES.md` and `FORMAT-DFA.md`.
The split point is dfa-games commit `ef1bcd7c23d47a979f5856e998a236d0b9cf8e2b`.

`config/` was copied from that commit, not imported with history: only the `positions-manual.json` and `positions-generated.json` files, which are all the Rust reads.
The other files under dfa-games `config/` serve the C++ solver and its symbolic move generation.
Both repositories keep these files at the same paths, so they can be compared or re-synced directly.
