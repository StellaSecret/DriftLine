# Design and validation

> For the precise, per-mechanic rules see [`docs/README.md`](docs/README.md). This file is the design rationale.

## Architecture

`field` (what the flow is) -> `sim` (deterministic RK4, region detection) -> `level` / `levels` (data) ->
`modes` (rules and interaction loop of each mechanic) -> `app` / `render` / `ui` (input and drawing).
Everything up to and including `modes` is windowless and unit-tested. Levels are plain data; fundamental mechanics,
constraints (target, exits, hazard, gate, several probes) and field properties (uniform, vortex, radial, saddle,
rotating, orbiting, modulated, wave, `Custom`) are independent axes, which is why combination levels are a few lines each.

## The mechanics (section 11 of the brief)

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

### Steer - "What should I do next?"
- **Information:** the complete static field, the fixed start, the visible target, and, after every beat, the probe's *actual* position, the path so far, the remaining gusts and the current natural velocity. The future of a candidate gust is never shown.
- **Decisions:** once per beat, a gust in one of 8 directions or nothing, with a limited gust budget.
- **Loop:** observe -> decide -> intervene -> advance one beat -> observe again.
- **Success:** the probe reaches the target within its tolerance before the beats run out. Leaving the bounds, running out of beats, or a numerical fault ends the run as a failure. Using up the gusts alone never fails a run.
- **Skill:** closed-loop control: choosing actions from observed state, where *when* and *what* you do both matter.
- **Different because:** Position and Tempo choose their one decision before the run, Predict commits to a forecast, and Experiment chooses parameters between runs. Steer is the only mechanic where the player acts during the run and sees the consequence before the next decision.

**Physics.** `dp/dt = v(p) + u`, where `u` is the gust vector for the current beat and zero otherwise. A gust is a velocity, never a displacement, and it does not persist. Steer calls the same RK4 sub-step (`sim::substep`) as `simulate`, which was extracted so both share one code path; with `u = 0` it is bit-identical to the plain field integration (tested). A beat is 1 s of simulation time (50 sub-steps of 0.02). Target contact tests the whole segment of each sub-step, so a fast probe cannot tunnel through a small target. That segment is the chord of one RK4 step, whose deviation from the true path is O(dt^2 x curvature), far below any tolerance used.

**State machine** (`modes/steer.rs`). `Ready -> Awaiting -> Advancing -> (Evaluating) -> Awaiting | Completed -> retry -> Ready`. `choose` is only accepted in `Ready`/`Awaiting` and moves to `Advancing`; everything else (a second gust, `retry`) is rejected until `finish_beat`, which is what the UI calls when the beat animation ends. That is how duplicate input is blocked. *Evaluating* happens atomically inside `finish_beat`, so no frame can observe it and it has no enum variant of its own. The pure function `SteerSpec::advance(state, action)` does the physics; the UI only ever calls `choose`, so it cannot preview an unchosen action.

**Retry, ghosts, replay.** `retry` restores the exact initial state and keeps a *completed* run as a ghost (the last 3 are kept; the current run is never truncated). Replay re-simulates the recorded actions rather than replaying stored points, and a test asserts the path is identical.

**Levels and validation** (`STEER_VALIDATION.md`, produced by `cargo run --bin report -- --steer`; every legal gust sequence is enumerated, at most 1,857 per level).

| Level | Config (beats / gusts) | What it teaches | Result |
|---|---|---|---|
| 1 First Gust | 5 / 1 | a gust changes velocity for one beat | natural path misses by 0.25; 10 of 41 sequences win; works at every beat |
| 2 Leverage | 6 / 2 | the same gust matters more in some places | leverage 7.0, 6.9, 5.6, 3.2, 1.4, 1.0; 76 solutions; one gust suffices only at early beats |
| 3 Tipping Point | 6 / 1 | timing matters near a separatrix | 6 solutions, all gusts in beats 1-3; no gust at beats 4-6 recovers |
| 4 The Long Way | 7 / 2 | the best move may be away from the target | needs both gusts; 94% of solutions open sideways or away from the target |
| 5 Set Up | 8 / 2 | an action can matter for what it enables | needs both gusts; 0 single-gust wins; 13 solutions; r = 0.3 |

For levels 2 to 5 the greedy bearing policy, the one-beat greedy lookahead and *any* sequence of toward-target gusts at any timing all fail. Only level 1 can be solved by toward-target gusts, which is fine for an introduction. No level is solved by doing nothing.

**Design method.** Level 3, 4 and 5 parameters were found by random search over field families and filtered with the same validator that now guards them: targets were constructed from the end of a known winning sequence, so solvability is guaranteed by construction and then checked against the rest of the criteria. The first drafts of levels 3 to 5 had zero solutions and were discarded, as were many candidates that were too permissive (over 100 solutions). The jet-between-vortices design for level 4 was dropped because its entry gust was only about 30 degrees from the target bearing and never counted as sideways; the orbit design replaced it.

**Known limitations.**
- Level 5's "the first gust sets up the second" is supported by the data (both gusts are required, no single gust works, the first gust is never the greedy move, and the best follow-up depends on the first), but the validator does not prove the mechanism, e.g. that the first gust moves the probe into a region of higher leverage. The leverage curve for level 5 is nearly flat.
- Level 3 has only 6 solutions, so it is strict. The same direction works at beats 1 to 3 and nothing recovers later.
- The greedy lookahead is a one-beat baseline. A deeper planner would of course solve these levels; that is the point of the puzzle.
- All five levels use 8 directions. 4-direction mode is implemented and tested but no level uses it.
- The abstractions `Knobs` and `Visibility` named in the brief do not exist in this codebase. Steer uses none of the release / intensity / phase controls and needs no visibility switch because everything is visible.

### Survey - "Where is the hidden source?" (prototype)
- **Information:** the charted background field (visible arrows), the zone, and every scout's finished 5 s trail with direction chevrons. The hidden source's position, and what it does to the field, are never shown before the commit.
- **Decisions:** where to release each of a limited number of scouts (one more is free), then where to put one estimate marker.
- **Loop:** observe -> place a scout -> read its trail -> place the next scout where the remaining explanations disagree -> commit one estimate -> reveal. A retry moves the source to a different authored position.
- **Success:** the committed marker is within 0.75 of the source.
- **Skill:** choosing which observation is most informative under a budget, i.e. inferring a hidden structure from sparse evidence.
- **Different because:** a scout's own outcome is irrelevant; it is an instrument. Experiment's runs are unlimited and test a *known* system for a relationship; here observations are limited and the unknown is a *structure*. By the check in `docs/08-comparison.md` it is only borderline (same lever as Position, same timing and observations as Experiment), so this is the least-proven mechanic: its evidence is an ideal solver, not players.

**The source** is `Term::Gauss`, a *local* repeller (`v = k d exp(-|d|^2/(2 sigma^2))`). The first design used `Term::Radial`, whose 1/r tail bends every scout wherever it is; a single free trail then settled 99.5% of sources, so there was nothing to decide. A local source can only be found by passing near it, which is what makes *where* to look matter.

**State machine** (`modes/survey.rs`). `Scouting -> Committed -> retry -> Scouting`. `retry` is refused until the commit, because abandoning a round would refund its scouts. `SurveySpec::fly` is the only function that sees the hidden position; the interface reads it only through `Round::revealed_source`, which is `None` before the commit.

**Validation** (`SURVEY_VALIDATION.md`, `cargo run --release --bin report -- --survey`; model in `validate_survey.rs`). The source is one of 221 candidates on a 0.5 grid; a scout's trail under each candidate is compared with what was seen, at two precisions (`0.15`: a careful player could see it; `0.4`: what mental inference can use). A level is *solvable* iff an oracle that knows the truth finds some combination of scouts that works (exhaustive, hard requirement); *non-trivial* is judged over many random sources and against baselines (fixed pattern, random scouts, an adaptive player that picks each scout to split the remaining candidates). The adaptive player is also run through the real `SurveyGame` and must agree with the model. A test checks the validator can say no (a source with no effect is rejected).

| Result (300 random continuous sources, 2 paid scouts) | coarse | fine |
|---|---|---|
| free scout alone | 39% | 57% |
| fixed pattern of 2 scouts | 75% | 92% |
| random scouts | 64% | 83% |
| adaptive, 1 scout / 2 scouts | 73% / 81% | 89% / 96% |
| unsolvable by any 2 scouts | 1% | 1% |

**Known limitations.**
- No human has played it. The question that decides whether Survey is a fundamental mechanic is whether players pick scouts to tell two explanations apart, not whether they find the source.
- The lead over a sensible fixed pattern is small (+6 points coarse, +4 fine); over random scouts it is +17 and +13. With 3 scouts the lead over the fixed pattern is +9 and +3.
- For ideal play 5 of the 6 authored positions need only one paid scout; the second matters for imperfect play (73% -> 81%).
- Difficulty is mostly whether the free trail (which only reaches x = 0) passes near the source. The two-scout positions all lie in one patch the free scout never sees.
- The validator's candidate grid is its own device; the game is continuous. Strength and width of the source are known; the estimate is a single point.

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
