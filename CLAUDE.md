# dfa-verify

An independent verifier for the output of the dfa-games solver (https://github.com/jconsidi/dfa-games).
dfa-games represents sets of game positions as layered DFAs: a position is a string of one character per board square, and a set of positions is the language of a DFA over that alphabet.
Its C++ solver builds those DFAs; this cargo workspace only *reads* them and checks them against game rules implemented here.
Nothing here constructs a DFA — building is the C++'s job.

This repository was split from dfa-games at commit `ef1bcd7c23d47a979f5856e998a236d0b9cf8e2b`, where it lived under `rust/`.
Doc comments and `TODO.md` still cite C++ sources as `src/…` (`src/DFA.cpp`, `src/verify_utils.cpp`, …); those paths are in the dfa-games repository, not here.

## Priorities

**Provable correctness overrides everything else here** — speed, convenience, API tidiness, and consistency with neighbouring code.
The solver computes answers nobody will check by hand, and this repository is the check, so a wrong result that looks like a right one is the worst thing it can produce.

**Always favor stopping with an error over silence.** In practice:

- A check that disproves what it was asked about returns an error.
  It must not return a verdict as a value the caller can drop — and do not offer a second, report-shaped entry point alongside, because that form can be called and ignored.
  Where statistics are also wanted, return them on the success path.
  `verify_dfa_union` (`dfa-format/src/union.rs`) is the worked example, and the reason: it once returned a report, a caller discarded it with `?`, and the union checks in `verify-backward-sound` passed a corrupted DFA while reporting success.
- Fail loudly on an unexpected condition rather than continuing with a plausible default or a quietly narrowed check.
- Never weaken or skip a check to make something build, pass, or run faster.
  Raise the tradeoff instead.
- Say what was checked.
  A check that prints nothing on success cannot be told apart from one that never ran.

## Building and testing

    cargo build --release && cargo test --release

Both crates build clean under `RUSTFLAGS="-D warnings" cargo clippy --release --all-targets`, and the workspace is `cargo fmt --all --check` clean.
Keep them both that way.
Clippy plays the role of the C++ side's `-Werror`; `fmt` is a gate so that a diff shows what changed rather than how it was wrapped.
Run

    cargo fmt --all && RUSTFLAGS="-D warnings" cargo clippy --release --all-targets

before committing, and never hand-wrap around rustfmt — if a line reads badly after formatting, change the code, not the whitespace.

`cargo test` needs nothing outside this repository.
It includes a config driven check of the rules themselves, `dfa-games/tests/config_positions.rs`, reading `config/<game>/` with one file per kind of check:

- `moves.json` — `expected_moves`, compared as a set.
- `perft_u.json` — `expected_perft_u`, distinct positions per depth, which covers every configured size without storing full move lists.
- `results.json` — `expected_result`, what `validate_result` returns: the result at a terminal position for the side to move, or null if the position is not terminal.
  It is not the value under optimal play; that is what the solver's DFAs claim and the verifiers check.

The schema is strict on purpose: an unknown or missing field, a duplicate position in a file, an unrecognized file under `config/`, or a game the Rust cannot build fails the run rather than being skipped, since test data that never executes is worse than none.
Keep it strict when adding a kind of check: a new file name, a new constant in the test, and a new arm in `scripts/import-positions.py`.

`scripts/import-positions.py <dfa-games checkout> <commit> [game ...]` imports positions from dfa-games, where new games usually get theirs first.
It reads that commit through git, splits each case by kind, and merges by position and side to move, stopping without writing anything on a conflict.
Positions added here have no `source`; imported ones carry `"source": "dfa-games <sha>"`.
Name the commit in the commit message of an import.

## Crates

- `dfa-format` — the single authority on the `.dfa` file format (`FORMAT-DFA.md`).
  `layout.rs` owns every byte offset, so the reader and writer cannot drift.
  `Dfa::positions()` in `iter.rs` is the port of `DFAIterator` and enumerates accepted strings in lexicographic order.
  Binaries: `dfa-validate`, `dfa-stats`, `dfa-verify-union`.
- `dfa-games` — position level rules (`validate_moves`, `validate_result`, `position_to_string`) and the verifiers over them.
  Binaries: `verify-lost-sound`, `verify-won-sound`, `verify-losing-sound`, `verify-winning-sound`, `verify-backward-sound`, taking the same arguments as their C++ namesakes.

## Running against solver output

Every binary takes `--scratch` (or `DFA_ARCHIVE_DIR`), defaulting to `scratch`, naming a dfa-games scratch directory.
DFAs there are content addressed in `dfas_by_hash/<sha256>.dfa`, with `<game>/<name>` symlinks per game.
On this machine one is at `~/projects/dfa-games/src/scratch`:

    target/release/verify-backward-sound --scratch ~/projects/dfa-games/src/scratch breakthrough_4x4 1

The scratch directory is a cache on the dfa-games side and gets cleared.
When symlinks dangle, the DFAs have to be regenerated there with the C++ (`./solve_backward breakthrough_4x4` from dfa-games `src/`); this repository cannot rebuild them.

## Rules and coverage

Coverage is narrower than the C++ and the gaps matter:

- **amazons**, **row-wise breakthrough**, **clobber**, **cram**, **normalnim**, **slidingtile** and **tictactoe** have Rust rules.
  slidingtile is a one-player puzzle (`Game::sides` is 1), so `cargo test` checks its moves but the verifiers refuse it: every claim they check is a two-player result.
  `breakthroughcw_`, `chess+` and `othello_` are recognized by name so the error says "not ported" rather than "unrecognized" — but they are genuinely not verifiable from here.
- The rules those modules implement are written down in `GAMES.md`, which describes the games and nothing else — no shapes, no square numbering, no file names.
  Rules for a new game come from there, or from the game itself, **not** from porting the C++ move graph: a transcription reproduces whatever the C++ gets wrong and then agrees with it.
  Only the encoding is read from the C++, because it is the interface the two sides must share; it belongs in the Rust module doc.
  `breakthrough.rs` and `amazons.rs` predate this and are ports, which `TODO.md` records.
- Solved sets contain positions unreachable from the starting position — both sides holding a winning condition at once, or the side to move holding one.
  Backward solving covers every position on purpose, since testing reachability in the set representation would cost more than solving the extras, which resolve within about 2 ply.
  Rules must not treat such a position as a failure: tictactoe stops the side to move only on the *opponent's* line for exactly this reason.
- Neither implementation's losing and winning verifiers consult `lost,side_to_move=N` or `won,side_to_move=N`.
  Both decide terminal positions from the rules, but they branch differently.
  The Rust generates each position's moves once, and an empty move list branches to `validate_result`, which must return the base case the DFA asserts.
  The C++ asks `validate_outcome` first: a terminal result must be -1 in a losing DFA and 1 in a winning DFA, and any other result fails; a position with no result has its moves checked.
  Neither verifier cross checks the `lost` DFA as a side effect, so keep running `verify-lost-sound` on `lost,side_to_move=N` itself.

The two implementations are meant to disagree loudly, so when changing either, check both against the same DFAs.
`verify-backward-sound breakthrough_4x4 1` here and `./verify_backward_sound breakthrough_4x4 1` in dfa-games `src/` cover eight DFAs in well under two minutes between them.
The messages differ by design — only the counts and the verdicts are comparable.

Enumeration costs roughly 100 ns per position on its own; the rules dominate after that, and by a lot.
Breakthrough is about 0.2 us per position, amazons about 16 us, so pick a test case by expected runtime rather than position count.

## Platforms

Development is mostly on macOS, with Linux also supported.
Treat Linux as comparatively untested, and do not assume behaviour holds on one platform without checking the other.

## Shell

The interactive shell is zsh, which does **not** word-split unquoted parameters.
`$FLAGS` passes as a single argument where bash would split it.
Use arrays or write flags literally.

## Commits

Prefer more, smaller commits over one large one, provided each is self-contained: it builds on its own and, where practical, passes `cargo test` on its own.
Split by concern — a rename, a manifest fix, a behaviour change — rather than squashing a session's work into a single commit at the end.
A commit that mixes an unrelated cleanup with a behaviour change makes both harder to review and harder to `git bisect` later, which matters more here than in most projects given the correctness bar in **Priorities**.

## Markdown Usage

When writing Markdown, write each sentence on a separate line.
This convention is to simplify diffs and avoid large changes from adjusting word wrapping.
Most Markdown renderers will treat consecutive lines as the same paragraph and format them normally.
