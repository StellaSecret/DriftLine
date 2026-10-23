# 08. The five mechanics side by side

All five use the same field, integrator and regions ([01-engine.md](01-engine.md)). What differs is **which input of the simulation the player controls, when they control it, and what they observe before the next decision**.

## 1. Timeline of a play

```
Position   [choose release point] --launch--> watch the whole run ------------------------> result
Predict    [commit a forecast] ---commit--> reveal the run -------------------------------> compare   (3 rounds)
Tempo      watch field ... [choose launch moment] --launch--> watch run (field keeps moving) -> result
Experiment [choose intensity] --run--> observe --> [choose intensity] --run--> ... --> [conclude] --> verdict
Steer      observe --> [gust or wait] --> 1 beat --> observe --> [gust or wait] --> 1 beat --> ... --> end
```

## 2. Comparison table

| | Position | Predict | Tempo | Experiment | Steer |
|---|---|---|---|---|---|
| **Question** | Where should I start? | Where will it go? | When should I launch? | How does the system behave? | What should I do next? |
| **What the player supplies** | initial position(s) | an answer (an exit) | an initial *time* | a parameter, then a conclusion | velocity impulses |
| **Input to the simulation** | start point | none | start clock | field intensity | extra velocity during chosen beats |
| **Decided** | before the run | before the reveal | before the run | between runs | during the run |
| **Number of decisions** | 1 (per attempt) | 3 (one per round) | 1 (per attempt) | many runs, then 1+ conclusions | up to `beats` (5 to 8) |
| **Can the player act on the run once it starts?** | no | no | no | no | **yes, every beat** |
| **Preview of the outcome** | none | none (a partial prefix in `cmb-2`) | none | none (runs are real, not previews) | none (the net-velocity arrow is not a trajectory) |
| **Information gained between decisions** | the finished trajectory | the true path and verdict | the finished trajectory (and the marks) | the stored runs | the actual new state after each beat |
| **Field during play** | fixed | fixed | changes with time | changes with the intensity | fixed |
| **What is hidden** | nothing | the path | nothing | the truth | nothing |
| **Win** | probe reaches goal (+ gates) | at least 2 of 3 forecasts correct | probe reaches goal | correct conclusion | probe reaches goal within the budgets |
| **Failure states** | none (a miss) | failing the 3 rounds | none (a miss) | none | leaving bounds, beats exhausted, hazard, numerical fault |
| **Retry** | free; keeps placements; 3 faint trails | after 3 rounds; **new** start points | free; marks on the timeline | free; runs kept (Reset clears them) | free; exact restart; 3 ghosts; replay |
| **Resource limits** | none | none | none | none (rig cap in `exp-4`) | beats and gust budget |
| **Randomness** | none | seeded choice of starts | none | none | none |
| **Code** | `modes/position.rs` | `modes/predict.rs` | `modes/tempo.rs` | `modes/experiment.rs` | `modes/steer.rs` |

## 3. What makes each one a different kind of reasoning

- **Position**: map initial conditions to outcomes by looking at the picture (spatial inference about a fixed flow).
- **Predict**: simulate in your head and commit. There is no feedback loop inside a round, and a wrong answer cannot be fixed.
- **Tempo**: relate *when* to *shape*: the same start produces different paths.
- **Experiment**: design tests and infer a rule; success is a statement about the system, not an outcome of a run.
- **Steer**: control. Plan a sequence of interventions, then correct it using the actual state after each step.

## 4. Where the distinctions are weaker than they sound

The first four mechanics share a structure: one decision, then a watched run. The reasoning differs (position vs. time vs. forecast vs. rule), but the *interaction loop* is similar, and in Position and Tempo unlimited retries allow trial-and-error instead of reasoning (see their documents, section 10). Steer is the only mechanic whose loop is structurally different. Experiment's conclusion step can currently be brute-forced (see [05-experiment.md](05-experiment.md)), which weakens its claim to "reward learning a rule rather than guessing".

## 5. A check for any new mechanic

A proposed mechanic is genuinely new only if, in the table above, it differs from every existing column in at least one of: **what is controlled**, **when**, or **what is observed before the next decision**; and it must still make sense when the field art is identical. Changing the field shape, the constraint (target, gate, hazard) or the budget does not make a new mechanic: those are content (see [07-combinations.md](07-combinations.md)).
