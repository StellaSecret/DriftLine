# 05. Experiment: "How does the system behave?"

Source: `src/modes/experiment.rs` (rules and ground truth), `src/app.rs::exp_frame` (interface). Levels `exp-1`..`exp-5`; also `cmb-4`.

## 1. At a glance

| | |
|---|---|
| Control | the field's **intensity** (a scalar scaling the tunable layers) |
| Decision | which intensities to test, then a *conclusion* about the system |
| When | between runs: run, observe, run again, then conclude |
| Feedback | every run's trajectory, kept on screen; a verdict on the conclusion |
| Attempts | unlimited runs; unlimited conclusions (see the weakness in section 10) |
| Win | a conclusion that matches the ground truth computed from the simulation |
| Fixed | start point, field structure, exits |

## 2. Information

Visible: the field (the arrow grid is drawn at the **currently selected** intensity, so it changes as the slider moves), the fixed start, all exits, the question, every earlier run with its intensity and outcome, and the evidence checklist.
Hidden: the ground truth. In `exp-4` the outcome at the target intensity cannot be run at all (the rig is capped).

## 3. Controls

| Input | Effect |
|---|---|
| Intensity slider (drag), `Left`/`Right` (held) | change the selected intensity. Keys move it by 25% of the range per second. The slider always spans the level's full range. |
| `Space` / `Enter` / Run | run the selected intensity (disabled while a run animates) |
| Click a row of the run list | show or hide that run's path |
| `R` / Reset runs | delete all runs |
| Conclusion buttons / estimate sliders, then Submit / Report finding | submit a conclusion (enabled only when the evidence is complete) |

The run list shows only the **four most recent** runs; older runs stay on screen while visible but cannot be toggled from the list.

## 4. State and loop

`ExperimentGame { runs, solved, feedback }`.

```
form a hypothesis -> choose an intensity -> Run -> observe (the path stays) -> ... -> evidence complete -> Submit conclusion -> verdict
```

There is no turn structure: runs and conclusions can be interleaved freely, and a wrong conclusion does not end anything.

## 5. Exact rules

### 5.1 A run

`run(intensity)`: clamp the intensity to the level's range `[lo, hi]`; if it exceeds the **rig limit** (the `limit` of an Extrapolate challenge, otherwise `hi`) reject it with "Beyond the test rig. Infer it from what you can run."; otherwise simulate from the fixed start with the clock at 0 and the field at that intensity (`probe`). The run `{intensity, trajectory, visible: true}` is appended. Runs are deterministic: the same intensity always gives the same path.

### 5.2 Evidence

A conclusion is accepted for checking only when **all** the level's evidence conditions hold (otherwise the verdict is "Gather more evidence first." and nothing is evaluated).

| Challenge | Evidence conditions |
|---|---|
| ExitShift | at least 3 runs; runs span at least 40% of the range (max intensity minus min intensity run) |
| Return | at least 5 runs; runs span at least 70% of the range |
| Extrapolate{limit,target} | at least 5 runs; runs span at least 60% of `limit`; at least one run with intensity at or above `0.66 * limit` |
| Threshold{from,to} | at least one run that left through `from`; at least one that left through `to` |
| Interval{exit} | at least one run through `exit`; at least one run that did **not**; at least 3 runs |

### 5.3 Ground truth (computed, never stored)

All truths sweep the real simulation. `sweep(n)` takes `n` intensities evenly spaced over `[lo, hi]` including both ends.

| Challenge | Truth |
|---|---|
| ExitShift{center} | `sweep(61)`; for each path take its **winding angle** about `center` (the sum of the wrapped angle increments between consecutive path points; positive = counter-clockwise). Compare consecutive samples: a difference above +0.001 counts as "up", below -0.001 as "down". Only "up": **counter-clockwise** (choice 1). Only "down": **clockwise** (choice 0). Otherwise (mixed or flat): **neither** (choice 2). |
| Return | `sweep(241)`; list the exits the runs left through, merging consecutive repeats and skipping runs that did not end at an exit. The answer is **yes** (choice 0) if some exit appears again after a different exit, else **no** (choice 1). |
| Extrapolate{target} | simulate at `target`: `Reached(e)` gives exit `e`; `Boundary` gives "leaves between exits"; anything else means no truth (the level would be invalid). |
| Threshold{from,to} | `sweep(401)`; find the first adjacent pair where the earlier sample left through `from` and the later one through `to`; refine the boundary by 40 bisections on "leaves through `from`". Directly adjacent exits are required: if there is none, there is no truth. |
| Interval{exit} | `sweep(401)`; the first and last samples that leave through `exit`. Each edge is refined by 40 bisections (or is `lo`/`hi` if the band touches the end of the range). |

Features narrower than one sweep step could be missed. The step is the range divided by n - 1: 0.033 (ExitShift in `exp-1`, Return in `exp-3`), 0.0075 (Threshold in `exp-2`, `cmb-4`), 0.0125 (Interval in `exp-5`).

### 5.4 Conclusions and verdicts

- Choices. ExitShift: "Clockwise", "Counter-clockwise", "Neither / inconsistent". Return: "Yes: it returns to an exit it left", "No: every exit is visited once". Extrapolate: one choice per exit plus "Leaves between exits" (the last index). (The button shows only the text before `:` or `/`.)
- Estimates (Threshold: 1; Interval: 2). The submitted values are sorted; the conclusion is correct iff the count matches and each value is within the level's tolerance of the corresponding truth value. The estimate sliders start at 33% and 66% of the range.
- `solved |= correct`. `solved` is **sticky**: `Reset runs` clears the runs and the feedback but not `solved`.

## 6. Success and failure

Success: a correct conclusion. There is no failure state and no limit on conclusions.

## 7. Invariants and edge cases

- Truth is computed from the same simulation the player runs, so a correct rule always exists and the verdict can never disagree with what the player could observe (checked by `experiment_truths_are_robust_and_specific`).
- The Extrapolate answer is robust: the outcome at `target` is unchanged for intensities within 0.15 of it.
- The selected intensity can sit beyond the rig limit (the arrows then show the stronger field), but running is refused.
- Running the same intensity twice adds a second identical run.

## 8. Feedback

After a run: "Run n: intensity x.xx -> <exit X | between exits | target | ...>." Verdicts: "Confirmed. Your rule matches the simulation."; for a wrong Threshold/Interval "Not within tolerance. Bracket the change with more runs, then narrow it."; for a wrong choice "That does not match the system. Compare your runs side by side before answering again.". Runs are drawn in distinct colours (a fixed cycle of 8), labelled at their end with "#n I=x.xx"; the run list shows a filled dot for visible runs and an empty circle for hidden ones. The evidence checklist shows a ticked box per satisfied condition. The banner reads "Experiments run: n".

## 9. Levels

All Experiment levels except `exp-2` use the same "spiral" field: a fixed outward radial push (k = 1.2) plus a tunable clockwise vortex (k = -1 x intensity), start (1.4, 0), four corner exits A (top right), B (bottom right), C (bottom left), D (top left), clockwise in that order of appearance as intensity grows: B, C, D, A, then B again. The radial escape does not depend on the intensity, so the winding angle grows in proportion to it.

| Level | Range | Challenge | Truth | Tolerance |
|---|---|---|---|---|
| `exp-1` Which Way? | [0, 2] | ExitShift | clockwise | n/a |
| `exp-2` The Switch | [0, 3] | Threshold A -> B (uniform flow plus a tunable counter-clockwise vortex; exits are two large touching discs) | 1.4809 | 0.15 |
| `exp-3` Round Trip | [0, 8] | Return | yes (B C D A B) | n/a |
| `exp-4` Beyond the Rig | [0, 8], rig stops at 4 | Extrapolate to 6.2 | exit B (the second pass) | n/a |
| `exp-5` The Window | [0, 5] | Interval for exit D | [2.7352, 3.8989] | 0.2 per end |
| `cmb-4` Three Exits | [0, 3] | Threshold C -> D | 2.7352 | 0.15 |

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md).

## 10. Distinctness and known weaknesses

- **Different because** the goal is a *rule about the system*, not a target: completion is never "a run hit the target", and the verdict comes from a sweep of the simulation.
- **Weakness 1: conclusions are free to retry.** The evidence gate only prevents concluding with *no* evidence; once it is met, a wrong conclusion has no cost. Measured: after 6 runs the choices of `exp-1` (3 choices), `exp-3` (2) and `exp-4` (5) can simply be tried in turn. At most 2, 1 and 4 wrong tries are needed. The continuous challenges can be brute-forced too: about 10 guesses for `exp-2` and `cmb-4` (range / (2 x tolerance)), and about 150 for `exp-5`, which needs two correct ends at once (about 12.5 steps per end).
- **Weakness 2: evidence is shallow.** The Threshold and Interval conditions only require having seen the relevant exits; they do not require runs close to the switch.
- **Strength.** `exp-4` is the strongest design: the target outcome cannot be run, so it must be inferred from a measured rate (the winding angle grows with intensity), although with 5 choices it can still be brute-forced.
- **Possible remedies (not implemented).** Allow one conclusion per evidence set, or reset the evidence after a wrong conclusion; require a run within the tolerance band of the reported value; limit the number of runs.

## 11. Tests

`experiment_conclusions_are_checked_against_the_simulation`, `experiment_requires_evidence_so_guessing_cannot_win` (it covers only the case of *no* runs, see weakness 1), `experiment_rig_cap_forces_extrapolation`, `experiment_truths_are_robust_and_specific`, `experiment_runs_are_preserved_and_comparable` (`tests/mechanics.rs`).
