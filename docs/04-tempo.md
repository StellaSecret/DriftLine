# 04. Tempo: "When should I launch?"

Source: `src/modes/tempo.rs` (rules), `src/app.rs::tempo_frame` (interface). Levels `tem-1`..`tem-5`; also `cmb-3`.

## 1. At a glance

| | |
|---|---|
| Decision | the moment (phase) at which to release the probe |
| When it is made | before the run, while watching the field evolve |
| Feedback | the trajectory, after the launch; the field keeps evolving during the flight |
| Attempts | unlimited; each launch is marked on the timeline |
| Win | the probe reaches the target (without touching the hazard in `cmb-3`) |
| Fixed | field structure, release point, target |

## 2. Information

Visible: the field **as it is at the clock's current time**, the fixed release point, the target (and hazard), the timeline with the period and the marks of earlier launches (a triangle for a success, a cross for a miss). Hidden: no trajectory preview exists.

## 3. Controls

| Input | Effect |
|---|---|
| Click or drag on the timeline | set the clock to that phase (pauses playback) |
| `Left` / `Right` | step the clock by -/+0.25 s (pauses playback) |
| `P` / Play-Pause button | toggle the field's real-time playback |
| `Space` / `Enter` / Launch | release the probe at the current clock phase |

Other controls are absent: there is no position, intensity or timing-speed control.

## 4. State and loop

`TempoGame { clock in [0, period), playing, attempts, won }`.

```
watch the field (clock runs while `playing`) -> scrub or wait -> launch -> watch the flight (field keeps evolving) -> repeat
```

`playing` starts true. While a flight animates, the clock does **not** advance; afterwards it resumes from the launch phase.

## 5. Exact rules

- `tick(dt)` (called every frame while no flight is animating): if `playing`, `clock = (clock + dt).rem_euclid(period)`. The clock therefore runs in real time at 1 field-second per real second.
- `set_phase(x)`: `clock = x.rem_euclid(period)`, `playing = false`.
- `launch()`: simulate from the fixed start with the field clock beginning at `t0 = clock`; the field time then continues to grow past the period, which is harmless because the fields are periodic with that period. `success = reached_goal(...)`; `won |= success`; the attempt `(phase, trajectory, success)` is recorded.
- The field's period is 12 s in every shipped level (`tem-1`..`tem-5`, `cmb-3`). Field time at release is the launch phase.

## 6. Success and failure

Success: `Reached(goal)` (and all gates, none here). A hazard ends the run as a miss (`cmb-3`). Leaving the bounds, `Timeout`, or `Stalled` are also misses. No attempt limit.

## 7. Invariants and edge cases

- The field is exactly periodic: launching at phase `p` and at `p + 12` gives the same trajectory (tested by `tempo_period_is_exact` to 1e-6).
- The displayed field during a flight is the field at `t0 + elapsed`, so the player sees it change under the probe.
- Launching while paused or while playing at the same phase gives the same result; "waiting" only changes which phase you land on.
- Playback and the flight animation use real time, but the *outcome* depends only on the phase: there is no reaction-time component beyond choosing when to press Launch.

## 8. Feedback

Success: "Success! The timing worked." Miss: "Missed: <how it ended>. Scrub the timeline and compare how the field looks." (a gate message exists for completeness). The panel lists the last four launches as "`<phase> s  reached|missed`". The timeline shows the clock cursor (yellow), tick marks each second, and the markers of earlier launches. The field label above the timeline reads the current phase.

## 9. Levels

"Windows" = contiguous (cyclic) ranges of successful launch phases, over 240 sampled phases; "success phase" = share of phases that succeed; "geometry spread" = share of 60 phases whose path differs from the phase-0 path by more than 1 unit at some of five equally spaced arc-length points (it shows the **shape** of the path depends on phase, not only its speed).

| Level | Field | Intent | Windows | Success phase | Geometry spread |
|---|---|---|---|---|---|
| `tem-1` Two Moods | uniform flow plus a square-wave vertical push | two clearly different phases | 1 | 32.1% | 66.7% |
| `tem-2` Slow Tide | uniform flow plus a sinusoidally modulated vertical push | a broad window | 1 | 31.7% | 68.3% |
| `tem-3` Knife Edge | same field, target on the axis | a narrow window (two of them, both short) | 2 | 9.2% | 70.0% |
| `tem-4` Double Beat | the push oscillates twice per cycle | several windows | 4 | 25.0% | 90.0% |
| `tem-5` Moving Vortex | a vortex circles the centre once per cycle | phase decides on which side the probe passes | 1 | 19.2% | 58.3% |
| `cmb-3` Dodge | `tem-2` plus a hazard | the hazard removes the early part of the window | 1 | 18.8% | 71.7% |

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md).

## 10. Distinctness and known weaknesses

- **Different because** the same start and the same field give different *paths* depending only on time, and the decision is *when*, not where.
- **Weakness.** The phase is a one-dimensional choice that can be swept: the timeline, unlimited retries and the marks of earlier launches make a systematic scan (try a phase, note the mark, try another) a viable strategy. The design counters it with narrow windows (`tem-3`: 9.2% of phases), not with limits.
- The probe's flight is long compared to the window width only in some levels, so "the launch moment" is really "the interval the flight occupies"; the windows reflect that.

## 11. Tests

`tempo_windows_match_the_level_design`, `tempo_changes_geometry_not_just_speed`, `tempo_period_is_exact`,
`tempo_mode_only_exposes_time`, `combination_levels_each_add_exactly_one_idea` (`tests/mechanics.rs`).
