# 08. The six mechanics side by side

All six use the same field, integrator and regions ([01-engine.md](01-engine.md)). What differs is **which input of the simulation the player controls, when they control it, and what they observe before the next decision**.

## 1. Timeline of a play

```
Position   [choose release point] --launch--> watch the whole run ------------------------> result
Predict    [commit a forecast] ---commit--> reveal the run -------------------------------> compare   (3 rounds)
Tempo      watch field ... [choose launch moment] --launch--> watch run (field keeps moving) -> result
Experiment [choose intensity] --run--> observe --> [choose intensity] --run--> ... --> [conclude] --> verdict
Steer      observe --> [gust or wait] --> 1 beat --> observe --> [gust or wait] --> 1 beat --> ... --> end
Survey     observe --> [place a scout] --> its trail --> [place a scout] --> ... --> [commit one estimate] --> reveal
```

## 2. Comparison table

| | Position | Predict | Tempo | Experiment | Steer | Survey |
|---|---|---|---|---|---|---|
| **Question** | Where should I start? | Where will it go? | When should I launch? | How does the system behave? | What should I do next? | where is the hidden source? |
| **What the player supplies** | initial position(s) | an answer (an exit) | an initial *time* | a parameter, then a conclusion | velocity impulses | scout release points, then one estimate |
| **Input to the simulation** | start point | none | start clock | field intensity | extra velocity during chosen beats | start point of each scout |
| **Decided** | before the run | before the reveal | before the run | between runs | during the run | between flights; the commit last |
| **Number of decisions** | 1 (per attempt) | 3 (one per round) | 1 (per attempt) | many runs, then 1+ conclusions | up to `beats` (5 to 8) | 2 scouts + 1 commit (1 more scout is free) |
| **Can the player act on the run once it starts?** | no | no | no | no | **yes, every beat** | no (each scout runs to its end) |
| **Preview of the outcome** | none | none (a partial prefix in `cmb-2`) | none | none (runs are real, not previews) | none (the net-velocity arrow is not a trajectory) | none (a scout is only flown when placed) |
| **Information gained between decisions** | the finished trajectory | the true path and verdict | the finished trajectory (and the marks) | the stored runs | the actual new state after each beat | each scout's finished trail |
| **Field during play** | fixed | fixed | changes with time | changes with the intensity | fixed | fixed (the charted background; the real field hides a source) |
| **What is hidden** | nothing | the path | nothing | the truth | nothing | the source's position |
| **Win** | probe reaches goal (+ gates) | at least 2 of 3 forecasts correct | probe reaches goal | correct conclusion | probe reaches goal within the budgets | estimate within 0.75 of the source |
| **Failure states** | none (a miss) | failing the 3 rounds | none (a miss) | none | leaving bounds, beats exhausted, hazard, numerical fault | none (a miss; the source is revealed) |
| **Retry** | free; keeps placements; 3 faint trails | after 3 rounds; **new** start points | free; marks on the timeline | free; runs kept (Reset clears them) | free; exact restart; 3 ghosts; replay | only after a commit; the source moves to another authored position |
| **Resource limits** | none | none | none | none (rig cap in `exp-4`) | beats and gust budget | 2 paid scouts, 1 commit |
| **Randomness** | none | seeded choice of starts | none | none | none | seeded order of 6 authored positions |
| **Code** | `modes/position.rs` | `modes/predict.rs` | `modes/tempo.rs` | `modes/experiment.rs` | `modes/steer.rs` | `modes/survey.rs` |

## 3. What makes each one a different kind of reasoning

- **Position**: map initial conditions to outcomes by looking at the picture (spatial inference about a fixed flow).
- **Predict**: simulate in your head and commit. There is no feedback loop inside a round, and a wrong answer cannot be fixed.
- **Tempo**: relate *when* to *shape*: the same start produces different paths.
- **Experiment**: design tests and infer a rule; success is a statement about the system, not an outcome of a run.
- **Steer**: control. Plan a sequence of interventions, then correct it using the actual state after each step.
- **Survey**: choose evidence. Decide what to observe, under a budget, to tell apart the places a hidden thing might be, then commit an estimate.

## 4. Where the distinctions are weaker than they sound

The first four mechanics share a structure: one decision, then a watched run. The reasoning differs (position vs. time vs. forecast vs. rule), but the *interaction loop* is similar, and in Position and Tempo unlimited retries allow trial-and-error instead of reasoning (see their documents, section 10). Steer is the only mechanic whose loop is structurally different. Experiment's conclusion step can currently be brute-forced (see [05-experiment.md](05-experiment.md)), which weakens its claim to "reward learning a rule rather than guessing".

Survey is the weakest-evidenced addition. Against the check in section 5 it controls a release point (as Position does), between flights (as Experiment's runs are), and observes finished trails (as Experiment does), so it is only borderline by that rule; what is meant to differ is the *role* of the decision (which observation to make, under a budget, about an unknown structure), which the check does not measure. Its evidence so far comes from an ideal solver, not from players: see [09-survey.md](09-survey.md), section 10.

## 5. A check for any new mechanic

A proposed mechanic is genuinely new only if, in the table above, it differs from every existing column in at least one of: **what is controlled**, **when**, or **what is observed before the next decision**; and it must still make sense when the field art is identical. Changing the field shape, the constraint (target, gate, hazard) or the budget does not make a new mechanic: those are content (see [07-combinations.md](07-combinations.md)).
