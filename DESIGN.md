# Design and validation

## Architecture

`field` (what the flow is) -> `sim` (deterministic RK4, region detection) -> `level` / `levels` (data) ->
`modes` (rules and interaction loop of each mechanic) -> `app` / `render` / `ui` (input and drawing).
Everything up to and including `modes` is windowless and unit-tested. Levels are plain data; fundamental mechanics,
constraints (target, exits, hazard, gate, several probes) and field properties (uniform, vortex, radial, saddle,
rotating, orbiting, modulated, wave, `Custom`) are independent axes, which is why combination levels are a few lines each.

## The four mechanics (section 11 of the brief)

### Position - "Where should I start?"
- **Information:** the whole fixed field, the target(s), the release zone.
- **Decisions:** only where in the zone to release (click or arrow keys).
- **Loop:** observe -> place -> launch -> watch -> adjust -> retry.
- **Success:** the probe reaches the target (and every gate) within tolerance.
- **Skill:** reasoning spatially about how initial conditions map to paths.
- **Different because:** the *only* lever is the initial state; the field never changes and there is nothing to forecast or discover.

### Predict - "Where will it go?"
- **Information:** the field, the start, the candidate exits. The path is hidden (or only its opening is shown in the partial-visibility combo).
- **Decisions:** one answer per round. Nothing else is controllable.
- **Loop:** observe -> commit forecast -> reveal -> compare dashed forecast with the true path.
- **Success:** at least 2 of 3 rounds correct. Guessing alone passes about 16% of the time (tested).
- **Skill:** reading arrows and extrapolating motion several steps ahead.
- **Different because:** no experiment is possible. A wrong answer is locked after commit, and a retry draws *new* release points, so it cannot become parameter search.

### Tempo - "When should I launch?"
- **Information:** the periodic field (animated, scrubbable), the fixed start, the target, marks for earlier launches.
- **Decisions:** only the launch phase.
- **Loop:** watch the field evolve -> scrub or wait -> launch -> watch the field keep changing during flight.
- **Success:** the probe reaches the target; windows range from broad to narrow and from one to several per cycle.
- **Skill:** relating phase to trajectory geometry.
- **Different because:** the same start and the same field give different *paths* by time alone. `tempo_changes_geometry_not_just_speed` checks that at least 40% of phases change path shape, and a periodicity test checks the field repeats exactly.

### Experiment - "How does the system behave?"
- **Information:** the known field and a tunable intensity; every earlier run stays visible in its own colour with its parameter.
- **Decisions:** which intensities to test, then a *conclusion* (a rule or an estimate).
- **Loop:** hypothesis -> choose a test -> run -> compare runs -> conclude.
- **Success:** the conclusion matches ground truth computed by sweeping the real simulation (direction of turn, whether exits recur, a switch value within tolerance, an interval, or the outcome beyond a capped test rig).
- **Skill:** investigating a system deliberately.
- **Different because:** completion is never "a run hit the target". Conclusions are refused until evidence exists (e.g. you must have *seen* both exits before reporting the threshold between them). In *Beyond the Rig* the target intensity cannot be run, so the exit follows only from the rule that the winding angle grows linearly with intensity.

## Combinations
| Level | Combines | Why it exists |
|---|---|---|
| cmb-1 | Position + gate | the gate removes about 65% of previously winning starts |
| cmb-2 | Predict + partial reveal | the opening of the path is evidence you must extrapolate from |
| cmb-3 | Tempo + hazard | the hazard removes the early part of the otherwise good window |
| cmb-4 | Experiment + several exits | the switch to find is between two specific exits among four |
| cmb-5 | Position + Tempo | only 33% of phases admit any winning start, so place and time interact |
| cmb-6 | Position + shared intensity (2 probes) | only 38% of dial settings let both probes win |

## Decisions where the brief was ambiguous
- **Predict start point:** "fixed" is per round. A level is 3 rounds with different fixed starts, 2 correct to pass, so a single lucky guess does not clear it.
- **Predict answers:** starts are drawn only where the exit is stable under +/-0.15 perturbation, so the correct answer is never a coin flip.
- **Experiment tolerance:** thresholds and intervals accept +/-0.15 to 0.2 intensity.
- **Gates** model the "corridor" constraint: a waypoint that must be passed before the goal counts.
- **Accessibility:** colour-blind-safe (Okabe-Ito) palette; every entity also has its own shape (diamond start, ringed target with cross, lettered exits, hatched hazard, dashed gate); full keyboard operation; high-contrast arrows (`K`).

## Measured by `cargo run --bin report`
Position win area: 100%, 20%, 19%, 20%, 6.5% (needle). Tempo launch windows per cycle: 1, 1, 2, 4, 1.
Every Predict level offers 3 to 4 distinct answers. Every Experiment truth is computed, not stored. All 26 levels are solvable.
