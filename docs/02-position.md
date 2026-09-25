# 02. Position: "Where should I start?"

Source: `src/modes/position.rs` (rules), `src/app.rs::pos_frame` (interface). Levels `pos-1`..`pos-5`; also used by `cmb-1`, `cmb-5`, `cmb-6`.

## 1. At a glance

| | |
|---|---|
| Decision | where to release each probe, inside its release zone |
| When it is made | before the run. Once launched, the run cannot be influenced |
| Feedback | the complete trajectory, after the launch |
| Attempts | unlimited. Placements are kept between attempts; the last 3 earlier attempts are drawn as faint trails |
| Win | every probe reaches its goal region (after visiting every gate) |
| Fail states | none: a launch that misses is just a miss |
| Fixed | field, intensity, launch phase (in the five fundamental levels) |

## 2. Information

Visible from the start: the whole field (arrow grid), every goal region, hazards and gates, every release zone.
Hidden: nothing. No preview of the trajectory exists: the player must infer it from the arrows.

## 3. Controls

| Input | Effect |
|---|---|
| Click inside a release zone | places that zone's probe at the **exact click position** (not snapped). A click outside every zone does nothing. Ignored while a run is animating. |
| Arrow keys (held) | move the selected probe at 1.2 world units/s (0.25 with Shift held), clamped to its zone. If nothing is placed yet the probe starts at the zone's centre. |
| `Tab` | (levels with several probes) select the next probe |
| `Space` / `Enter` / Launch button | launch (only when every probe is placed) |
| Combination levels only: timeline click/drag, `,` and `.` | launch phase: set it / step it by 0.25 s |
| Combination levels only: slider, `-` and `=` | shared intensity: set it / step it by 0.1 |

In the five fundamental levels there is no phase control and no intensity control.

## 4. State and loop

`PositionGame { placed: Vec<Option<V2>>, phase, intensity, attempts, won }`.

```
observe -> place (or move) probe(s) -> launch -> watch the animation -> (adjust and repeat)
```

- Placing or nudging after a launch clears the displayed result and animation (the placements of other probes stay).
- `launch()` returns `None` unless every slot has a placed probe.
- There is no terminal state: after winning, the player can keep launching ("Keep playing").

## 5. Exact rules

- **Construction.** One slot per release zone, each with its own goal region. `placed` starts empty. `phase` starts at 0. `intensity` starts at the field's default (1), or at the midpoint of the intensity range if the level has a shared-intensity control.
- **`place_at(p)`.** Finds the **first** slot whose zone contains `p` and stores `p` there; returns its index (or `None`). Zones in the shipped levels never overlap.
- **`nudge(slot, d)`.** `placed[slot] = zone.clamp(current_or_zone_centre + d)`.
- **`set_phase(x)`.** Only if the level has a period: `phase = x.rem_euclid(period)`. **`set_intensity(x)`** only if the level has an intensity range: clamps to that range.
- **`launch()`.** For every slot independently: simulate from the placed point with the field at the chosen intensity and the clock starting at `phase` (`Level::run`), then `slot_ok = reached_goal(trajectory, slot.goal)`. The attempt succeeds iff **all** slots succeed. `won |= success`. The attempt (starts, trajectories, per-slot flags, phase, intensity) is appended to `attempts`.
- The probes do not interact; each is a separate simulation.

## 6. Success and failure

A probe succeeds iff its outcome is `Reached(goal)` for **its own** goal and every gate was visited (`Level::reached_goal`). Anything else (a hazard, leaving the bounds, `Timeout`, `Stalled`, reaching the wrong region) is a miss for that probe. There is no attempt limit and no scoring beyond "launches used", shown on the completion banner.

## 7. Invariants and edge cases

- Same placement + phase + intensity always gives the same trajectory.
- A placement on the exact edge of a zone is inside (inclusive containment).
- With several probes, one success and one failure is a failed attempt; the per-probe results are listed in the message.
- `won` is sticky: it stays true after later misses.

## 8. Feedback

Success: "Success! Every probe reached its target." Failure: for one probe "Result: <how it ended>."; for several "Probe 1: ... Probe 2: ..." where a miss reads one of `target`, `exit X`, `hazard`, `between exits` (left the bounds), `timed out`, `came to rest`, `error`, or "reached the target but skipped the gate". The placed probe is a white diamond with an orange disc; with several probes each is numbered and coloured; a finished trajectory is drawn in the probe colour with an end ring; earlier attempts are drawn faintly (opacity 0.22), the last three at most. The banner reads "Launches used: n".

## 9. Levels

Win fraction = share of a 41 x 41 grid of release points (inside the zone) that wins. "Path diversity" = number of distinct (reached?, x-side, y-side) end sectors over a 21 x 21 grid.

| Level | Intent | Win fraction | Path diversity |
|---|---|---|---|
| `pos-1` Open Water | broad zone, obvious solution (an attractor at the target) | 100.0% | 2 |
| `pos-2` Two Roads | a clockwise vortex splits the flow; different starts take different paths | 19.9% | 4 |
| `pos-3` Saddle Point | the target is reachable from only part of the zone (trajectories follow hyperbolas `(x - cx) y = const`) | 19.2% | 3 |
| `pos-4` Mirror Mirror | mirror-symmetric field with an asymmetric goal: the mirror of a winning start ends in a hazard | 19.8% | 3 |
| `pos-5` The Needle | a small target (r = 0.3) beyond a saddle; a thin winning band, measured 0.12 to 0.13 units wide (about 10 px on screen; the arrow keys are the practical way to hit it) | 6.5% | 3 |
| `cmb-1` Through the Gate | `pos-2` plus a gate that removes most winners | 7.1% | 4 |

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md).

## 10. Distinctness and known weaknesses

- **Different because** the only lever is the initial state: the field never changes, nothing is forecast and nothing is discovered.
- **Weakness.** Placement is unlimited-retry, so a patient player can find a winning point by trial and error (a "slider until it works" strategy) instead of reasoning. The design counters this only by making the winning set small (about 7% to 20% of the zone) and not by limiting attempts. The faint trails help comparison, which also helps trial and error.
- `pos-1` is intentionally trivial (every start wins).

## 11. Tests

`position_difficulty_curve_and_different_starts_differ`, `position_mirror_level_is_symmetric_but_goal_is_not`,
`position_mode_rejects_off_zone_clicks_and_requires_all_probes`, `combination_levels_each_add_exactly_one_idea`
(all in `tests/mechanics.rs`).
