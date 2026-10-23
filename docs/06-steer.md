# 06. Steer: "What should I do next?"

Source: `src/modes/steer.rs` (rules), `src/sim.rs::{substep, advance_beat}` (physics), `src/validate_steer.rs` (analysis), `src/app.rs::steer_frame` (interface). Levels `ste-1`..`ste-5`.

## 1. At a glance

| | |
|---|---|
| Decision | every beat: apply a gust in one of 8 directions, or do nothing |
| When | **during the run**: the simulation pauses after each beat and waits |
| Feedback | after every beat, the probe's actual new state (position, path so far, budget, field velocity) |
| Resources | `beats` (maximum number of beats) and `gust_budget` (maximum number of gusts) |
| Attempts | unlimited; each retry restarts from the exact initial state and keeps the finished run as a ghost |
| Win | the probe reaches the target within its radius before the beats run out |
| Fixed | field, start, target, beat length (1 s), gust strength |

## 2. Information

Visible at every decision: the whole static field, the target and its tolerance circle, the probe's current position, the path so far with a tick at every beat boundary, the beat counter, the remaining gust budget, the natural field velocity at the probe, and the currently selected action with the resulting net velocity.
**Never shown:** the consequence of any candidate gust. The interface only ever calls `choose` for the action the player commits to; the physics function `SteerSpec::advance` is not used for previews. (The validation tools call it, but they are not part of the game.)

## 3. Controls

| Input | Effect |
|---|---|
| Compass buttons or `Q W E / A S D / Z X C` | select NW, N, NE / W, Wait, E / SW, S, SE. The centre (`S`) is "Wait". |
| Arrow keys | select N, S, W, E directly (gust directions need at least one gust left) |
| `0` or numpad `0` | select Wait |
| `Space` / `Enter` / Confirm | commit the selection and advance one beat. **A selection is required**: there is no default action. |
| `R` / Retry | restart from the initial state (disabled while a beat is advancing or a replay is running) |
| `V` / Replay | replay the finished run (or, mid-run, the latest ghost); `V` again stops it |

Gust buttons are disabled when the budget is empty; Wait is always available. While a beat is advancing, or while a replay is running, all compass buttons and Confirm are disabled. There is no time limit and no input is time-critical.

## 4. State and loop

```
Ready --choose--> Advancing --finish_beat--> Awaiting --choose--> Advancing ...  --finish_beat--> Completed(Success | Failed(reason))
  ^                                                                                                      |
  +----------------------------------------------- retry ----------------------------------------------+
```

| State (`Phase`) | Meaning | Accepts |
|---|---|---|
| `Ready` | initial state, nothing chosen | `choose` |
| `Awaiting` | the probe is stationary; a beat has finished | `choose` |
| `Advancing` | a decision was committed and its beat is being shown | nothing: `choose` and `retry` return `Busy` |
| `Completed(end)` | the run is over and frozen | `retry` (`choose` returns `Finished`) |

The specification's *Evaluating* step happens atomically inside `finish_beat`, so no frame can observe it and it has no state of its own. `Ready` and `Awaiting` behave identically for input.

## 5. Exact rules

### 5.1 The beat (`SteerSpec::advance(state, action)`, pure)

1. `legal`: in this order, if the run has an end then `Finished`; `Wait` is legal; `Gust(i)` with `i` outside the level's direction set is `BadDirection`; `Gust` with no gusts left is `NoBudget`.
2. The gust vector `u = unit(i) * strength` (zero for Wait). Directions in index order (counter-clockwise from east):

   | Set | Directions |
   |---|---|
   | 8 directions | 0 E (1,0), 1 NE (s,s), 2 N (0,1), 3 NW (-s,s), 4 W (-1,0), 5 SW (-s,-s), 6 S (0,-1), 7 SE (s,-s), with `s = 1/sqrt(2)` |
   | 4 directions | E, N, W, S |

   All directions have the same length `strength`.
3. Integrate `dp/dt = v(p, t) + u` over exactly one beat (`advance_beat`: 50 RK4 sub-steps of 0.02 s, the same `u` in every sub-step) from the current position, with `t = beat_index * beat_time` as the clock (irrelevant for static fields). Region contact, gates, bounds and numerical validity are checked in every sub-step exactly as in any other run (see [01-engine.md](01-engine.md)).
4. The probe's new position is the last recorded sub-step (the start position if there were none). The beat counter increases by 1; the gust budget decreases by 1 if the action was a gust.
5. The run's end is decided from the integrator's outcome, **in priority over the beat count**:

   | Outcome during the beat | Result |
   |---|---|
   | reached the goal region and every required gate was visited | `Success` |
   | reached the goal without the gates | `Failed(MissedGate)` |
   | reached another Target/Exit region | `Failed(WrongRegion)` |
   | hit a hazard | `Failed(Hazard)` |
   | left the bounds | `Failed(Boundary)` |
   | invalid value or speed above 50 | `Failed(Invalid)` |
   | nothing, and this was the last beat | `Failed(BeatsExhausted)` |
   | nothing, and beats remain | the run continues |

   Consequently reaching the target during the **final** beat is a success, and success mid-beat ends the beat early (its point list is shorter).

### 5.2 The game (`SteerGame`)

- `choose(a)`: rejected with `Busy`/`Finished` unless the phase is `Ready` or `Awaiting`; otherwise it runs `advance`, **spends the gust immediately** (so the budget display updates at once), stores the computed beat as pending and enters `Advancing`. Errors from `advance` (`BadDirection`, `NoBudget`) leave the state unchanged.
- `finish_beat()`: only in `Advancing`. Commits the pending beat (position, beat count, gates, end), appends it to the history, and moves to `Awaiting` or `Completed`. A second call returns nothing. `won` becomes true on the first success and stays true.
- `step(a)` = `choose` + `finish_beat` (used by tests, search and headless play).
- `retry()`: `Busy` while advancing. A **completed** run is first appended to the ghost list (at most `MAX_GHOSTS = 3`; the oldest is dropped). Then the state, history and pending beat are reset to the exact initial state, the phase becomes `Ready`, and `attempts` increases by 1 (even if the run was abandoned before ending; an abandoned run is **not** kept as a ghost).
- `replayable()`: the current run if completed, otherwise the latest ghost.
- **Replay** (`SteerSpec::replay(actions)`): re-simulates the recorded action list from the initial state, so a replay reproduces the physics instead of replaying stored pixels. The same field, start, beat length and actions give an identical path (tested).

### 5.3 Quantities shown at a decision

- Field velocity: `v(p)` at the probe's current position.
- Gust velocity: `u` of the selected action (zero if Wait or nothing selected).
- Net velocity: their vector sum. This is the instantaneous velocity at the start of the beat, **not** a trajectory.

## 6. Success and failure

Success: the probe reaches the target (radius = tolerance) during a beat; the run freezes at once with no further action needed. Failure: leaving the bounds, using all beats without reaching it, hitting a hazard, or a numerical fault. **Using up the gust budget is never a failure** by itself: the probe may still reach the target on the natural field.

## 7. Invariants and edge cases

- A gust is a **velocity**, never a displacement: the first sub-step moves the probe by `(v + u) * 0.02`, and it never jumps (tested by `a_gust_changes_velocity_not_position`).
- A gust lasts exactly one beat (`a_gust_disappears_after_its_beat`); waiting is bit-identical to the plain field integration (`no_gust_reproduces_the_natural_trajectory`).
- The gust is consumed even if the run ends during that beat.
- A rejected request (no budget, bad direction, duplicate) changes nothing, not even the beat counter.
- Duplicate input during a beat cannot advance it twice (`duplicate_input_cannot_advance_the_simulation_twice`).
- Contact detection tests the swept segment, so a fast probe cannot pass through a small target between samples (`success_is_detected_mid_beat_and_does_not_tunnel`).
- Gust strength is the same for every direction in a level; beat length is 1 s in every level; strength is 1.0 except `ste-2` (0.35).
- Four-direction mode is implemented and tested but no shipped level uses it.

## 8. Feedback

**Decision overlay** (at the probe, only when input is accepted): a blue **field** arrow from the probe; a dashed yellow **gust** arrow starting at the *tip* of the field arrow; a white **net** arrow from the probe to the combined tip. Arrow length is 0.5 world units per unit of speed (so a field of speed 2 is drawn 1 unit = 80 px long). Each arrow is labelled in text, so colour is not the only cue.
**Panel:** "Beat k of N" (k = completed beats + 1 while the run is live), gust pips (filled = available) and "n left", numeric Field / Gust / Net components, "Selected: ...".
**Beat track** under the play area: one cell per beat, with an arrow glyph pointing in the gust direction for gust beats and a dot for waits; the live beat is outlined.
**Path:** orange with a small ring at each beat boundary; every gust beat has a yellow marker (a dot and an arrow) at the position where it was applied, labelled with its beat number.
**Animation:** each beat is shown over 0.9 s (a fraction of the beat's recorded points per frame); input stays locked until it ends.
**Ghosts:** earlier completed runs are drawn faintly (path opacity about 0.19 to 0.30 with three ghosts, 0.30 with one; the oldest is the faintest) with their beat ticks and gust markers.
**Replay:** the re-simulated run is drawn in full colour, beat by beat at the same 0.9 s per beat, then held for 1.2 s and closed. The live selection is untouched.
**Messages** on completion: "Reached the target on beat k using g gust(s)."; "The probe left the field."; "Out of beats before reaching the target."; "Numerical error: the run was stopped."; "The probe hit a hazard."; "Reached the target but skipped the gate."; "The probe ended in the wrong region.". The banner reads "Gusts used: g of G   Attempts: n".

## 9. Levels

Measured by `cargo run --release --bin report -- --steer` (every legal gust sequence enumerated; a run that ends early counts once). "Greedy" columns: pushing toward the target every beat (nearest of the 8 directions to the bearing); the one-beat-lookahead policy (the action minimising the distance at the end of the beat); and **any** sequence whose every gust points toward the target at the moment it is used, at any timing.

| Level | Beats / gusts | Field | Sequences | Solutions | Min gusts | Natural miss | Greedy bearing / lookahead / any toward-target |
|---|---|---|---|---|---|---|---|
| `ste-1` First Gust | 5 / 1 | nearly uniform | 41 | 10 | 1 | 0.25 | fails / fails / **solves** |
| `ste-2` Leverage | 6 / 2 | divergent zone in a stream | 993 | 76 | 1 | 2.16 | fails / fails / fails |
| `ste-3` Tipping Point | 6 / 1 | uniform flow plus a saddle | 49 | 6 | 1 | 1.74 | fails / fails / fails |
| `ste-4` The Long Way | 7 / 2 | anticlockwise vortex plus inflow | 1401 | 18 | 2 | 1.04 | fails / fails / fails |
| `ste-5` Set Up | 8 / 2 | two vortices, a repeller, drift | 1857 | 13 | 2 | 2.06 | fails / fails / fails |

Per-level properties that carry the lesson:

- **`ste-1`** teaches "a gust changes velocity for one beat": 25% of the 40 single-gust placements win (north-ish gusts at any of the five beats); the field is uniform, so the leverage is exactly 1.0 at every beat.
- **`ste-2`** teaches that location matters. Leverage per beat (shift of the final position divided by the gust's nominal displacement, maximised over directions): 7.02, 6.93, 5.63, 3.24, 1.41, 1.00. A single gust wins only at the three highest-leverage beats (4% of single placements); some high-leverage first gusts have no winning continuation; the best second gust differs between first gusts (9 distinct follow-up sets).
- **`ste-3`** teaches timing near a separatrix: only gusts in beats 1 to 3 (of 6) can win; no gust at beat 4 or later recovers (6 solutions in all). The greedy choice never opens a winning sequence.
- **`ste-4`** teaches an indirect route: both gusts are required (0% single-gust wins) and 94% of solutions start with a gust at least 80 degrees away from the bearing to the target (away or sideways).
- **`ste-5`** teaches setup: both gusts are required (0% single-gust wins), no solution starts with the greedy first move, and there are 6 distinct follow-up sets. The target radius is 0.3.

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md). Full report: [../STEER_VALIDATION.md](../STEER_VALIDATION.md).

## 10. Distinctness and known weaknesses

- **Different because** it is the only mechanic where the player acts *during* the run and sees the real consequence before the next decision: observe, decide, intervene, advance, observe again.
- **Weakness 1:** "the first gust sets up the second" in `ste-5` is supported by the numbers above but not proven mechanistically; the leverage curve there is nearly flat (about 2.5).
- **Weakness 2:** `ste-3` has only 6 solutions, so it is strict.
- **Weakness 3:** the greedy baselines are one-beat policies; a deeper planner would of course solve the levels.
- **Weakness 4:** there is no cost to retrying, and ghosts show a previous attempt, so a systematic retry can learn a solution by trial and error (at most 1,857 sequences in the largest level). The closed loop does mean each attempt carries real information (the actual state), unlike a one-shot guess.
- `ste-1` can be solved by always pushing toward the target; it is an introduction.

## 11. Tests

`tests/steer.rs`: `no_gust_reproduces_the_natural_trajectory`, `a_gust_changes_velocity_not_position`, `a_gust_disappears_after_its_beat`, `net_velocity_is_the_vector_sum_and_gusts_have_equal_magnitude`, `replay_reproduces_the_same_trajectory`, `gust_budget_cannot_go_negative`, `doing_nothing_costs_nothing_and_using_all_gusts_is_not_failure`, `success_is_detected_mid_beat_and_does_not_tunnel`, `bounds_invalid_state_and_exhausted_beats_end_the_run`, `duplicate_input_cannot_advance_the_simulation_twice`, `retry_restores_the_initial_state_and_keeps_a_ghost`, `steer_chapter_matches_the_spec_configuration`, `no_level_is_solved_by_doing_nothing`, `level1_...`, `level2_...`, `level3_...`, `level4_...`, `level5_...`, `validation_report_covers_every_level`.
