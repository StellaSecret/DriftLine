# 09. Survey: "Where is the hidden source?"

Source: `src/modes/survey.rs` (rules), `src/field.rs::Term::Gauss` (the hidden source), `src/validate_survey.rs` (analysis), `src/app.rs::survey_frame` and `src/render.rs::{draw_zone_frame, draw_chevrons, draw_estimate, draw_source}` (interface). Level `sur-1`. Generated numbers: [`SURVEY_VALIDATION.md`](../SURVEY_VALIDATION.md).

**Status: prototype.** One level, validated against an *ideal* solver. No human has played it, and whether players choose scouts for what they can teach (rather than scattering them) is the open question. See section 10.

## 1. At a glance

| | |
|---|---|
| Decision | where to release each scout (any point in the zone), then where to put one estimate marker |
| When | scouts: one at a time, each seen to fly before the next is placed; the commit: once, last |
| Feedback | each scout's trail (5 s, shown in full, with direction chevrons) |
| Resources | **1 free scout** (flown at the start of every attempt) + `scouts` paid scouts (2 in `sur-1`) + **one** commit |
| Attempts | unlimited; a retry is only possible after a commit and hides the source somewhere else |
| Win | the committed marker is within `tol` (0.75) of the hidden source |
| Fixed | the charted background field, the source's strength `k` (1.6) and width `sigma` (1.0), the trail length, the free scout's start |
| Hidden | the source's position (and so everything its push does to the field) |

## 2. Information

The field in the level data is the **charted background**: a steady stream, `(1, 0)` everywhere, drawn as ordinary arrows (`G` changes their density, `K` makes them high-contrast). The real field is that stream plus one hidden local repeller (`Term::Gauss`) at an unknown position. The arrows never include it.

Visible at all times: the arrows, the dashed frame of the zone, the free scout's start (`F`) and trail, every scout placed so far (a numbered start point and its full trail with direction chevrons), the estimate marker with a ring showing the commit tolerance, the scouts left, the attempt number.

**Never shown before the commit:** the source position, its effect on the field, the outcome of any scout that has not been placed. A scout is only simulated when the player places it (`SurveyGame::place_scout`); there is no preview. `Round::revealed_source()` returns `None` until the round is committed, and the interface only reads the source through it.

**Speed.** The chevrons are spaced by *distance along the path* (every 1.4 world units), not by time, so they show direction without showing where the scout sped up. Speed is still partly visible: every trail lasts the same 5 s, so its length is its mean speed times 5.

**Push arrows.** Along each trail the interface draws small arrows every 0.3 s wherever the scout felt at least `PUSH_MIN` = 0.2 of extra push: the scout's measured velocity (`SurveySpec::push_samples`, a centred difference over +-0.1 s) minus the charted flow at that point. That extra term is the hidden source's own push, which points straight away from it (and is longer closer in, until the source's core), so lines drawn back along the arrows meet at the source. It depends on the trail and the charted field only, never the hidden position. A legend under the field explains it. Hovering the field with a tool active shows a faint ghost of what a click would place (a scout's start point, or the marker with its tolerance ring, or a red cross where a scout would be refused); it never shows a trail.

*The sensing range is the difficulty knob.* A scout shows arrows only where the push is at least `PUSH_MIN`, about 2 units from the source (strength 1.6, sigma 1). Simulated with a 12 degree reading error, success within 0.75 by how the evidence was gathered:

| `PUSH_MIN` | free scout alone | + 1 scout placed blindly | + 2 scouts placed blindly | + 2 scouts within 1.8 of the source |
|---|---|---|---|---|
| 0.12 (first value) | 56% (4 of 6 positions solved at 80-88%) | 66% | 71% | 92% |
| **0.2 (shipped)** | **0%** | 18% | 36% | 90% |

At 0.12 most of the level was solved by the free scout, so paid scouts were redundant: the "fit-dominant" failure. At 0.2 the free scout rules out nothing but a band near its own line (no arrows is itself evidence: the source is more than about 2 from y = 0), a blind scout rarely helps, and a scout sent toward where the source must be does. The question the player faces is where to send the next scout. Test `a_scout_near_the_source_locates_it_and_the_free_scout_alone_never_does`. Not yet measured: how well the Test estimate overlay can locate a source from the free trail alone; the arrows cannot, and the overlay should be checked the same way before the level is called final.

*Why the arrows are local and not the end-point displacement.* A first version drew, for each trail, a ring where the scout would have ended with no source and a dashed arrow from it to the real end. It looked right and was not usable: measured against an ideal fit (which recovers all three of a player's missed attempts to 0.07), triangulating the end displacements gives a median error of 1.99 and 1% within 0.5, because a trail integrates the push along a curving path, so its net displacement does not point at the source. Players' misses (1.69, 1.30, 0.93) matched what that readout supports. The local push locates the source exactly (`a_scout_near_the_source_locates_it_and_the_free_scout_alone_never_does`). With a per-trail reading error of 8 / 15 / 25 degrees, simulated success within 0.75 is about 84-91 / 64-73 / 42-43 %, within 0.5 about 71-81 / 47-51 / 25 %: this is why the tolerance is 0.75.

**Test estimate (a budget).** With a marker placed, `T` or the **Test estimate** button flies every scout placed so far again *as if the source were at the marker* (`SurveyGame::test_estimate`) and shows the result as thin dashed trails next to the solid ones, with a small diamond marking which estimate they belong to. There are `TESTS_PER_ROUND` = 3 per attempt, refilled on retry; testing the same marker with the same scouts again is free (it would show the same thing), and a new scout makes the old result stale. The result is a snapshot: moving the marker does not update it. A test is a pure function of the marker and the scouts' start points, never of the hidden position (`a_test_needs_a_marker_and_never_reads_the_hidden_source` runs two rounds with different hidden positions and compares), so it checks a guess against evidence the scouts already gave and adds none.

The budget is a design choice with a stated cost. An unlimited, live overlay turns the level into sliding the marker until two curves match, which makes the scouts matter only when the free trail says too little and replaces the question "where should I send the next scout?" with "where should I drag the marker?". A budget makes *which estimate to test* a decision. The downside is that it borders on Experiment's loop (form a hypothesis, spend a trial, read the result); if Survey is meant to stay distinct from Experiment, the alternatives are showing only an end-point or a mismatch number per test, or dropping the overlay. Not playtested.

**Rays (reading aid, per level).** With `rays` on (`Rules::Survey`, `SurveySpec::push_rays`), every second push arrow also gets a long faint line running back along its own direction, toward the source. Lines from one trail, or from several, cross at the source, so the answer can be read off without extending arrows by eye (`rays_run_back_along_the_push_through_the_source`: each ray passes within 0.02 of the source). Search is unchanged: a scout still has to pass within its sensing range (`PUSH_MIN`) before there is anything to extend, and the free scout alone still locates none of the six positions. Level 1 is the teaching level and has rays on; later Survey levels should set `rays: false` so the player extends the arrows themselves. There is only one Survey level today, so the off state is covered by a unit test but has not been seen in play.

After the commit: the source is drawn (a disc with eight rays, ringed at `sigma`), with a line from the marker to it and the text "The source was at (x, y)".

## 3. Controls

| Input | Effect |
|---|---|
| Click in the field | uses the active tool at that point (place a scout, or place/move the marker) |
| `S` / `M` / `Tab`, or the **Scout** / **Marker** buttons | choose the tool (`Tab` toggles) |
| Arrow keys | move the keyboard crosshair (3.5 units/s; hold `Shift` for 0.8) |
| `Space` / `Enter` | use the active tool at the crosshair |
| `C` / **Commit position** | lock in the marker (needs a marker); reveals the source |
| `R` / Retry | start the next attempt (enabled only after a commit) |
| `V` / Replay | fly all of this round's trails together again |
| `T` / **Test estimate** | spend one of the attempt's 3 tests on the marker (needs a marker; see section 2) |
| `H` / `G` / `K` / `Esc` | hint, arrow density, high contrast, menu (as everywhere) |

After the last paid scout is placed the tool switches to the marker by itself. While a scout is flying, input is ignored. Clicking outside the dashed frame, on top of an earlier scout, or with no scouts left explains why nothing happened and spends nothing.

## 4. State and loop

```
Scouting --place_scout (while budget)--> Scouting
Scouting --set_marker--> Scouting (marker moves freely)
Scouting --commit (needs a marker)--> Committed
Committed --retry--> Scouting (next hidden position, only the free scout flown)
```

| State | Accepts | Rejects |
|---|---|---|
| `Scouting` (`verdict == None`) | `place_scout`, `set_marker`, `commit` | `retry` -> `NotFinished` |
| `Committed` (`verdict == Some`) | `retry` | `place_scout` -> `Finished`, `set_marker` / `commit` -> `Finished` |

`retry` is refused mid-round on purpose: abandoning a round would refund its scouts and allow unlimited scouting.

## 5. Exact rules

**The hidden source.** `Term::Gauss { c, k, sigma }` adds `v = k d exp(-|d|^2 / (2 sigma^2))` with `d = p - c`, a radial push away from `c` for `k > 0`. It is zero at the centre, peaks at distance `sigma` with speed `k sigma e^(-1/2)`, and falls off fast: 45% of the peak at 2 sigma, 5.5% at 3 sigma, 0.22% at 4 sigma, 0.003% at 5 sigma. With `k = 1.6`, `sigma = 1` the peak is 0.97, close to the background speed 1.0. It is the reason the mechanic works: `Term::Radial` has a 1/r tail that bends every scout a little wherever it is, so one scout already pins the source down (see the design evidence at the end of section 10). A local source can only be detected by passing near it.

**A scout.** `SurveySpec::fly(source, start)` simulates the charted field plus the hidden source from `start` at clock 0, with the level's bounds, no regions, and the level's time limit (`max_time = 5 s`) as the trail length. A trail ends early only by leaving the bounds. It is deterministic, and independent of every other scout (tested).

**Placing a scout** (`place_scout`), in this order: round committed -> `Finished`; no paid scouts left -> `NoScouts`; start outside the zone -> `OutOfZone`; within `MIN_SCOUT_SEPARATION` (0.05) of an earlier scout's start, the free one included -> `Duplicate` (it would repeat that trail). A rejected placement changes nothing. The zone is the rectangle x in [-5.5, 5.5], y in [-3.5, 3.5].

**The marker** (`set_marker`) is clamped into the zone and can be moved any number of times until the commit.

**The commit** (`commit`) needs a marker. `Verdict { distance, ok }` with `distance = |marker - source|` and `ok = distance <= tol` (inclusive). `won` becomes true on the first `ok` and stays true.

**The hidden position of each attempt** (`SurveySpec::source_for_attempt`): the level lists `pool` (6 authored positions) **in the order a player should meet them, easy to hard**. The first pass uses the pool in that order (a test checks it is non-decreasing in candidates left after the free scout, then scouts needed). Every later pass is a seeded Fisher-Yates permutation of the pool (one per pass, from `level.seed`), and never opens on the position the previous pass ended on. So each position is used once per pass, no attempt repeats the one before, and the same level always gives the same sequence. A retry cannot be won by remembering the previous answer. Before this, one seeded order was used from the start, and it put the two hardest positions (130 candidates left after the free scout) first and second.

**The free scout** is flown at the start of every attempt from `free` (-5, 0). It travels right with the stream for 5 s, so it only reaches x = 0: a source in the right half of the field leaves it untouched.

## 6. Success and failure

Success: the commit is within 0.75 of the source. Failure: any other commit; the source is revealed, the player may retry. There is no other failure state. The level is complete after the first success; further attempts are allowed.

## 7. Invariants and edge cases

- The source is unreadable before the commit (`revealed_source` is `None`); tested.
- Scouts are independent and deterministic: placing scout B never changes scout A's trail; tested.
- The free scout is not counted against the budget and is never removable.
- A source never brings a scout to rest (a repeller pushes away), so no single scout can come to rest on it and give its position away. Tested over every lattice scout and every authored position (`no_scout_ever_comes_to_rest...`). An *attracting* source would not be safe in general: `Outcome::Stalled` at the source is a free answer.
- A won level stays won when a later attempt is missed.
- Positions in the pool are inside the zone, at least 1.5 apart, and off the validator's 0.5 candidate grid (tested), so the validator cannot flatter itself.
- Only the free scout's start is fixed; a paid scout placed exactly there is `Duplicate`.

## 8. Feedback

Messages (`app.rs::survey_place`, `survey_commit`):

| Event | Message |
|---|---|
| hit | "Found it: your marker is 0.31 from the source (within 0.75)." (green; level-complete banner) |
| miss | "Missed: your marker is 1.42 from the source (needs within 0.75). Retry (R) hides it somewhere else." (red) |
| commit without a marker | "Place the marker first (M, then click)." |
| no scouts left | "No scouts left. Use the marker (M) to say where the source is." |
| outside the zone | "Place scouts inside the dashed frame." |
| on top of a scout | "A scout is already there: it would draw the same trail." |

Colours and shapes: the free scout is light grey and labelled `F`; paid scouts use the shared run palette and are labelled `1`, `2`; the estimate is a violet diamond with a tolerance ring; the revealed source is a vermillion disc with rays. Scout identity does not rely on colour alone (labels, start squares).

Hints (`H`, in order): the arrows are only the charted background and anything unexpected is the source; a scout far from the source learns almost nothing; put the next scout where two possible explanations would disagree. None gives the answer.

## 9. Levels

| Level | Configuration | Intent |
|---|---|---|
| `sur-1` Something in the Stream | uniform stream (1, 0); source k = 1.6, sigma = 1; 1 free + 2 paid scouts; 5 s trails; tolerance 0.75; 6 authored positions | first contact: learn that a trail only speaks for the places it went near |

The six hidden positions, in play order (easy to hard), are (-3.25, -2.25), (-2.75, 2.25), (-0.75, -2.25), (-0.25, 2.25), (3.25, -1.75), (2.25, 2.25). They were chosen from 140 candidate positions on a 0.5 lattice offset by (0.25, 0.25): 63 pass the validator's requirements (solvable at both precisions, free scout alone does not settle it, still ambiguous afterwards). Of those, **55 are settled by the adaptive player with one paid scout and only 8 need two, and all 8 lie in one small patch** (x in [2.25, 3.25], y in [1.25, 2.25]), where the free trail sees nothing. The pool takes one of them, (2.25, 2.25). Measured per position and over 300 random sources: [`SURVEY_VALIDATION.md`](../SURVEY_VALIDATION.md).

## 10. Distinctness and known weaknesses

**What it is meant to add.** Every other mechanic asks the player to *get a probe somewhere*, or to run tests on a *known* system. In Survey the scouts' own outcomes do not matter; they are instruments, and success is a statement about a hidden thing. The decision is *which observation to make*, under a budget, given what is already known.

**Against the project's own check** (`08-comparison.md`, section 5): Survey controls a release point (like Position), between flights (like Experiment's runs), and observes finished trails (like Experiment). By the letter of that check it is therefore **borderline**. What differs is the role of the decision (choosing evidence under a limited budget about an unknown *structure*, where Experiment's runs are unlimited and about a known system with an unknown *relationship*), and the check does not measure that. This is the weakest-supported claim here and only playtesting can decide it.

Known weaknesses, with numbers (300 random continuous sources, `SURVEY_VALIDATION.md`):

1. **Evidence is from an ideal solver, not a person.** The "adaptive player" knows the candidate set and computes which scout splits it best. It shows the level *can* reward deliberate placement; it does not show that players reason that way. `adaptive_play` runs the same player through the real `SurveyGame` using only the trails it returns, and a test checks it agrees with the model.
2. **A sensible fixed spread is nearly as good.** At the coarse precision (what a person can use) the adaptive player succeeds on 81% of random sources, two scouts at a fixed pattern on 75%, random scouts on 64%; at the fine precision 96%, 92% and 83%. The lead over a fixed pattern is only 6 points (4 fine). With 3 scouts it is 9 (3). It is clear against random scattering (+17, +13). An earlier estimate of +17 over the fixed pattern came from 120 sources and was noise.
3. **The budget is slack for ideal play.** On the authored positions the adaptive player needs one paid scout on 5 of 6; only (2.25, 2.25) needs both. For a player who does not know the answer it is a real constraint over random sources (one scout 73%, two 81%).
4. **Difficulty is mostly "did the free trail get near it?".** The free scout only reaches x = 0, and the hardest positions lie in the one patch it never sees. Easy positions are those the free trail already bends around.
5. **The free scout alone settles 39% of random sources (57% fine).** Authored positions are chosen where it does not (tested), but a *random* hidden position would often make the level trivial; that is why the pool is authored, and why any new level needs the validator.
6. **Retries cycle through only 6 positions.** A player who retries more than six times will see positions repeat (in a new order each pass).
7. **The estimate is a single point; strength and width are known.** Uncertainty about strength would make trails fit several locations for reasons unrelated to the intended lesson, so it is excluded from this prototype.
8. **The validator's candidate grid is its own device.** Candidates sit on a 0.5 grid; the game's solution space is continuous. Success in the validator means the centroid of the surviving candidates is within tolerance, a stand-in for a good human estimate.
9. **Trail length leaks mean speed** (section 2), and the 5 s window means a scout can only be informative about the 5 or so units it travels.
10. **The two precision thresholds are choices.** `EPS_FINE` = 0.15 (a difference a careful player could see) and `EPS_COARSE` = 0.4 (the precision of *mental* inference). The coarse one is a difficulty parameter to calibrate by playtest, and "maximum deviation along the trail" is a simple stand-in for what people notice. At the fine precision the level is much easier: the free scout alone settles 57% of random sources and a fixed pattern 92%.
11. **The tolerance is calibrated on a handful of misses, not a playtest.** The first value, 0.5, was a guess. Six consecutive misses by one player landed at 0.52, 0.55, 0.61, 0.66, 0.90 and 1.05 (found the region, not the point). 1.0 (= sigma) was tried and rejected: the free scout alone then fully settles (-2.75, 2.25), which the level forbids. 0.75 keeps all six positions valid; ideal-play success moves from 81% to 93% (coarse) and fixed pattern from 75% to 81%. The Test estimate overlay is the other half of the answer: it gives a person a way to reach precision that mental inference does not.

Why a local source, and why sigma = 1.0 (design evidence from a width scan: 120 sources, 3 scouts, coarse precision; results listed as adaptive / fixed pattern / random): a 1/r repeller made the free scout alone solve 99.5% of sources; a Gaussian with sigma = 1.2 gave 88 / 87 / 82 (almost no reason to place scouts carefully); sigma = 1.0 gave 89 / 72 / 66 (at 120 sources; see 2 above); sigma = 0.8 gave 82 / 63 / 54; sigma = 0.6 gave 71 / 38 / 37 but was too hard for a first level.

## 11. Tests

`tests/survey.rs` (33 tests) and four front-end tests in `src/app.rs`.

| Area | Tests |
|---|---|
| The term | `gauss_is_a_local_radial_push_peaking_at_sigma`, `the_true_field_is_the_chart_plus_the_source`, `a_distant_source_leaves_a_scout_on_the_charted_path_and_a_near_one_bends_it`, `no_scout_ever_comes_to_rest_so_a_sink_cannot_give_the_answer_away` |
| The rules | `a_round_starts_with_only_the_free_scout_and_nothing_revealed`, `the_scout_budget_is_enforced_and_the_free_scout_is_not_part_of_it`, `scouts_must_be_inside_the_zone_and_may_not_repeat_a_trail`, `scouts_are_deterministic_and_do_not_affect_each_other`, `the_source_stays_hidden_until_the_commit`, `a_commit_needs_a_marker_and_freezes_the_round`, `the_marker_can_move_until_commit_and_is_clamped_to_the_zone`, `the_verdict_is_decided_by_the_commit_tolerance`, `a_retry_needs_a_commit_and_hides_the_source_somewhere_else`, `every_authored_position_is_used_once_per_pass_and_the_first_pass_is_in_authored_order`, `a_win_is_never_lost_by_playing_on`, `different_hidden_positions_give_different_free_trails` |
| The level | `survey_is_registered_as_a_chapter_with_one_level`, `the_level_parameters_are_consistent` |
| Reading the evidence | `push_arrows_point_away_from_the_source_and_vanish_without_one`, `a_scout_near_the_source_locates_it_and_the_free_scout_alone_never_does` |
| Estimate check | `a_test_needs_a_marker_and_never_reads_the_hidden_source`, `a_test_on_the_true_source_reproduces_the_real_trails_and_a_wrong_one_does_not`, `tests_are_a_budget_that_the_same_estimate_does_not_spend_and_a_retry_refills`, `rays_run_back_along_the_push_through_the_source`, `the_commit_tolerance_is_the_calibrated_one` |
| Validation | `the_authored_order_goes_from_easy_to_hard`, `the_charted_flight_does_not_depend_on_the_hidden_source`, `every_hidden_position_is_solvable_within_the_budget_at_both_precisions`, `the_free_scout_never_settles_the_puzzle_by_itself`, `the_budget_is_a_real_constraint_for_a_player_who_does_not_know_the_answer`, `an_adaptive_player_wins_every_position_through_the_real_game_and_agrees_with_the_model`, `thoughtful_placement_beats_uninformed_placement_across_many_sources`, `the_validator_can_say_no` |
| Front end (no window needed) | `placing_the_last_scout_switches_to_the_marker_tool`, `rejected_placements_explain_themselves_and_cost_nothing`, `the_marker_tool_places_and_moves_the_estimate_without_spending_scouts`, `committing_without_a_marker_says_so_and_does_not_end_the_round`, `a_hit_shows_the_banner_and_a_miss_does_not`, `replay_gathers_every_trail_of_the_round_with_the_free_scout_distinguished` |

**How the validator works** (`src/validate_survey.rs`). Candidates for the source: 221 points on a 0.5 grid. A scout at lattice point `s` would draw a different trail under each candidate; two candidates are *indistinguishable from `s`* when their trails (34 samples, one per 0.15 s) never differ by more than `eps`. The evidence a scout gives is the set of candidates whose predicted trail matches the observed one (the best fits if none does). Players compared on the same sources: a **fixed pattern**, **random scouts**, an **adaptive** player (each scout chosen to minimise the expected number of remaining candidates), and an **oracle** that knows the truth and tries every combination of lattice scouts. A level is *solvable* iff the oracle wins; that is a hard requirement. *Non-triviality* is evaluated over a distribution of sources, not one.
