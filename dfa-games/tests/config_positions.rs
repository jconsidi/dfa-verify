//! Configured positions checked against the rules.
//!
//! Each `config/<game>/` holds up to three files, one per kind of check:
//!
//! - `moves.json`: `"expected_moves"`, the positions one move away, compared as a set.
//! - `perft_u.json`: `"expected_perft_u"`, the number of distinct positions at each depth, sides
//!   alternating, as `test_perft_u` counts them.
//! - `results.json`: `"expected_result"`, the result at a terminal position from the point of view
//!   of the side to move (-1, 0 or 1), or null for a position that is not terminal.  This is the
//!   game's end condition, not the value under optimal play.
//!
//! Every case has `"position"` (the vector representation) and `"side_to_move"`, the one field
//! its file checks, and optionally `"comment"` and `"source"` (set by
//! `scripts/import-positions.py`).  Any other field fails the run, as does a missing one, a
//! position listed twice in the same file, or a file under `config/` that is not one of these
//! three.  A file naming a game these rules cannot build also fails rather than being skipped.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use dfa_games::game::Game;
use dfa_games::get_game;
use serde_json::{Map, Value};

const MOVES: &str = "moves.json";
const PERFT_U: &str = "perft_u.json";
const RESULTS: &str = "results.json";
const KINDS: [&str; 3] = [MOVES, PERFT_U, RESULTS];

fn config_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is dfa-games/, and config/ is at the repo root.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../config")
}

fn game_dirs() -> Vec<(String, PathBuf)> {
    let dir = config_dir();
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("could not scan {}: {e}", dir.display()));

    let mut output = Vec::new();
    for entry in entries {
        let entry = entry.expect("could not read a config directory entry");
        let game_name = entry
            .file_name()
            .into_string()
            .expect("config directory name is not valid UTF-8");
        output.push((game_name, entry.path()));
    }

    output.sort();
    output
}

/// Every game directory carrying `filename`.
fn games_with(filename: &str) -> Vec<(String, PathBuf)> {
    game_dirs()
        .into_iter()
        .map(|(game_name, dir)| (game_name, dir.join(filename)))
        .filter(|(_, path)| path.exists())
        .collect()
}

fn characters(value: &Value, what: &str) -> Vec<u32> {
    let array = value
        .as_array()
        .unwrap_or_else(|| panic!("{what} is not an array: {value}"));

    array
        .iter()
        .map(|c| {
            let n = c
                .as_u64()
                .unwrap_or_else(|| panic!("{what} holds a non-character {c}"));
            u32::try_from(n)
                .unwrap_or_else(|_| panic!("{what} holds an out of range character {n}"))
        })
        .collect()
}

/// A position must fit the game before it can say anything about the rules.
fn check_shape(game: &dyn Game, position: &[u32], what: &str) {
    let shape = game.shape();
    assert_eq!(
        position.len(),
        shape.len(),
        "{what} has {} characters, but {} positions have {}",
        position.len(),
        game.name(),
        shape.len()
    );

    for (i, (&character, &size)) in position.iter().zip(shape).enumerate() {
        assert!(
            character < size,
            "{what} has character {character} at index {i}, outside the alphabet of size {size}"
        );
    }
}

fn as_set(moves: &[Vec<u32>], what: &str) -> BTreeSet<Vec<u32>> {
    let set: BTreeSet<Vec<u32>> = moves.iter().cloned().collect();
    assert_eq!(
        set.len(),
        moves.len(),
        "{what} lists the same position more than once"
    );
    set
}

/// Distinct positions at each depth from `position`, as `test_perft_u`
/// counts them with `get_moves_forward`.
fn perft_u(game: &dyn Game, side_to_move: u32, position: &[u32], depth: usize) -> Vec<usize> {
    let mut output = Vec::with_capacity(depth);
    let mut frontier: BTreeSet<Vec<u32>> = BTreeSet::from([position.to_vec()]);
    for ply in 0..depth {
        let side = (side_to_move + ply as u32) % game.sides();
        frontier = frontier
            .iter()
            .flat_map(|p| game.validate_moves(side, p))
            .collect();
        output.push(frontier.len());
    }
    output
}

/// The parts every case has, after checking its fields against the one
/// `expected` field its file allows.
struct Case<'a> {
    label: String,
    position: Vec<u32>,
    side_to_move: u32,
    expected: &'a Value,
}

fn parse_case<'a>(
    game: &dyn Game,
    filename: &str,
    index: usize,
    case: &'a Value,
    expected_field: &str,
) -> Case<'a> {
    let label = format!("{} {filename} case {index}", game.name());
    let case: &Map<String, Value> = case
        .as_object()
        .unwrap_or_else(|| panic!("{label} is not an object"));

    for (field, value) in case {
        match field.as_str() {
            "position" | "side_to_move" => {}
            "comment" | "source" => assert!(value.is_string(), "{label} {field} is not a string"),
            f if f == expected_field => {}
            other => panic!("{label} has unexpected field \"{other}\" ({filename} holds only \"{expected_field}\")"),
        }
    }

    let position = characters(
        case.get("position")
            .unwrap_or_else(|| panic!("{label} has no \"position\"")),
        &format!("{label} position"),
    );
    check_shape(game, &position, &format!("{label} position"));

    let side_to_move = case
        .get("side_to_move")
        .and_then(|s| s.as_u64())
        .unwrap_or_else(|| panic!("{label} has no \"side_to_move\""));
    assert!(
        side_to_move < u64::from(game.sides()),
        "{label} has side_to_move {side_to_move}, but the game has only {} side(s)",
        game.sides()
    );

    let expected = case
        .get(expected_field)
        .unwrap_or_else(|| panic!("{label} has no \"{expected_field}\""));

    Case {
        label,
        position,
        side_to_move: side_to_move as u32,
        expected,
    }
}

fn check_moves(game: &dyn Game, case: &Case) {
    let label = &case.label;
    let expected_moves: Vec<Vec<u32>> = case
        .expected
        .as_array()
        .unwrap_or_else(|| panic!("{label} expected_moves is not an array"))
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let move_out = characters(m, &format!("{label} expected move {i}"));
            check_shape(game, &move_out, &format!("{label} expected move {i}"));
            move_out
        })
        .collect();

    let moves = game.validate_moves(case.side_to_move, &case.position);
    let found = as_set(&moves, &format!("{label} generated moves"));
    let wanted = as_set(&expected_moves, &format!("{label} expected_moves"));

    if found != wanted {
        let mut report = format!(
            "{label} move mismatch, side to move {}:\n{}",
            case.side_to_move,
            game.position_to_string(&case.position)
        );
        for missing in wanted.difference(&found) {
            report += &format!("MISSING:\n{}", game.position_to_string(missing));
        }
        for extra in found.difference(&wanted) {
            report += &format!("UNEXPECTED:\n{}", game.position_to_string(extra));
        }
        panic!("{report}");
    }
}

fn check_perft_u(game: &dyn Game, case: &Case) {
    let label = &case.label;
    let expected_perft_u: Vec<usize> = case
        .expected
        .as_array()
        .unwrap_or_else(|| panic!("{label} expected_perft_u is not an array"))
        .iter()
        .map(|n| {
            let n = n
                .as_u64()
                .unwrap_or_else(|| panic!("{label} expected_perft_u holds a non-count {n}"));
            usize::try_from(n)
                .unwrap_or_else(|_| panic!("{label} expected_perft_u holds an out of range {n}"))
        })
        .collect();
    assert!(
        !expected_perft_u.is_empty(),
        "{label} expected_perft_u is empty, which checks nothing"
    );

    let counts = perft_u(
        game,
        case.side_to_move,
        &case.position,
        expected_perft_u.len(),
    );
    assert_eq!(
        counts,
        expected_perft_u,
        "{label} perft_u mismatch, side to move {}\n{}",
        case.side_to_move,
        game.position_to_string(&case.position)
    );
}

fn check_result(game: &dyn Game, case: &Case) {
    let label = &case.label;
    let expected_result = match case.expected {
        Value::Null => None,
        value => {
            let result = value
                .as_i64()
                .unwrap_or_else(|| panic!("{label} expected_result is not a number or null"));
            assert!(
                (-1..=1).contains(&result),
                "{label} expected_result is {result}, not -1, 0, 1 or null"
            );
            Some(result as i32)
        }
    };

    let moves = game.validate_moves(case.side_to_move, &case.position);
    let result = game.validate_result(case.side_to_move, &case.position, &moves);
    assert_eq!(
        result,
        expected_result,
        "{label} result mismatch, side to move {}: got {result:?}, expected \
         {expected_result:?}\n{}",
        case.side_to_move,
        game.position_to_string(&case.position)
    );
}

fn check_kind(filename: &str, expected_field: &str, check: fn(&dyn Game, &Case)) {
    let games = games_with(filename);
    assert!(
        !games.is_empty(),
        "no {filename} found under {}",
        config_dir().display()
    );

    let mut cases_run = 0usize;
    for (game_name, path) in &games {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        let config: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("could not parse {}: {e}", path.display()));

        let configured = config
            .get("game")
            .and_then(|g| g.as_str())
            .unwrap_or_else(|| panic!("{} has no \"game\"", path.display()));
        assert_eq!(
            configured,
            game_name,
            "{} is for {configured} instead of {game_name}",
            path.display()
        );

        // Not a skip: a file for a game with no rules here is a check that
        // would never run, which is what this is meant to prevent.
        let game = get_game(game_name).unwrap_or_else(|e| {
            panic!(
                "{} names a game these rules cannot build: {e:#}",
                path.display()
            )
        });

        let cases = config
            .get("positions")
            .and_then(|t| t.as_array())
            .unwrap_or_else(|| panic!("{} has no \"positions\" array", path.display()));
        assert!(
            !cases.is_empty(),
            "{} has no positions; delete it instead",
            path.display()
        );

        let mut seen = HashSet::new();
        for (index, case) in cases.iter().enumerate() {
            let case = parse_case(game.as_ref(), filename, index, case, expected_field);
            assert!(
                seen.insert((case.position.clone(), case.side_to_move)),
                "{} lists the same position and side to move more than once",
                case.label
            );
            check(game.as_ref(), &case);
            cases_run += 1;
        }
    }

    println!(
        "checked {cases_run} positions from {filename} across {} games",
        games.len()
    );
}

#[test]
fn config_holds_only_known_files() {
    let games = game_dirs();
    assert!(
        !games.is_empty(),
        "no games under {}",
        config_dir().display()
    );

    for (_, dir) in games {
        assert!(dir.is_dir(), "{} is not a game directory", dir.display());

        let mut found = 0;
        for entry in std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("could not scan {}: {e}", dir.display()))
        {
            let entry = entry.expect("could not read a config directory entry");
            let name = entry.file_name();
            assert!(
                KINDS.iter().any(|kind| name == *kind),
                "{} is not one of {KINDS:?}, so nothing would check it",
                entry.path().display()
            );
            found += 1;
        }
        assert!(found > 0, "{} is empty", dir.display());
    }
}

#[test]
fn moves_match_the_rules() {
    check_kind(MOVES, "expected_moves", check_moves);
}

#[test]
fn perft_u_matches_the_rules() {
    check_kind(PERFT_U, "expected_perft_u", check_perft_u);
}

#[test]
fn results_match_the_rules() {
    check_kind(RESULTS, "expected_result", check_result);
}
