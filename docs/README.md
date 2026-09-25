# Mechanics reference

Precise, code-derived documentation of how each fundamental mechanic works. It answers "what exactly happens
when the player does X" and "what exactly can the player do", with enough detail to reimplement the behaviour.

| Document | Content |
|---|---|
| [01-engine.md](01-engine.md) | The shared foundation: coordinates, field model, RK4 integrator, regions and outcomes, level data, UI conventions |
| [02-position.md](02-position.md) | **Position**: "Where should I start?" |
| [03-predict.md](03-predict.md) | **Predict**: "Where will it go?" |
| [04-tempo.md](04-tempo.md) | **Tempo**: "When should I launch?" |
| [05-experiment.md](05-experiment.md) | **Experiment**: "How does the system behave?" |
| [06-steer.md](06-steer.md) | **Steer**: "What should I do next?" |
| [07-combinations.md](07-combinations.md) | How the six combination levels compose the mechanics |
| [08-comparison.md](08-comparison.md) | The six mechanics side by side, and how they differ |
| [09-survey.md](09-survey.md) | **Survey**: "Where is the hidden source?" |
| [LEVEL_CATALOG.md](LEVEL_CATALOG.md) | **Generated**: every parameter of every level (fields, regions, rules) |

## How each mechanic document is organised

Every mechanic document follows the same sections, so they can be compared directly:
1. **At a glance**: question, decision, timing, feedback, retry, win/fail, code location.
2. **Information**: exactly what is visible and what is hidden.
3. **Controls**: every input that exists, by mouse and by keyboard.
4. **State and loop**: the states, transitions and guards.
5. **Exact rules**: the rules as the code implements them, with formulas and constants.
6. **Success and failure.**
7. **Invariants and edge cases.**
8. **Feedback**: what the interface shows and the exact messages.
9. **Levels**: design intent and measured properties of each level.
10. **Distinctness and known weaknesses**: what makes the mechanic different, and where that claim is weaker than it sounds.
11. **Tests**: the tests that pin the behaviour.

## Conventions

- Code references look like `src/modes/steer.rs::SteerSpec::advance`.
- "Tested by `name`" refers to a test in `tests/` or a unit test in `src/`.
- **What is enforced.** `LEVEL_CATALOG.md` is generated from the level data and a test (`tests/docs.rs`) fails when it is stale.
  Everything else here is hand-written from the source and is not enforced; statistics come from
  `cargo run --release --bin report` (and `-- --steer`, `-- --survey`) on this version of the code. If you change behaviour, update the
  matching document.
- Units: positions are in world units, durations in seconds, angles in radians. World x points right, y points up.
  (The screen y axis points down; the interface converts.)

## Glossary

| Term | Meaning |
|---|---|
| **Probe** | The moving point. It has a position only: no mass, inertia or momentum. |
| **Field** | A velocity function `v(p, t)`. The probe's velocity is the field's value at its position. |
| **Layer / term** | One summand of a field. A *tunable* layer is multiplied by the intensity. |
| **Intensity** | A scalar that scales the tunable layers. Fixed in most mechanics; the controlled variable in Experiment. |
| **Region** | A disc with a role: target, exit, hazard or gate. Its radius is its tolerance. |
| **Outcome** | How a simulated run ended: reached a region, hit a hazard, left the bounds, timed out, stalled, or invalid. |
| **Trajectory** | The sampled path of a run (one point per 0.02 s). |
| **Release point / start** | Where the probe begins. |
| **Phase / clock** | The field time at release. Periodic fields repeat after their period. |
| **Window** | A contiguous range of launch phases that succeed (Tempo). |
| **Round / forecast** | One Predict question; the player's committed answer to it. |
| **Rig** | The Experiment test apparatus; some levels cap the intensity it can run. |
| **Truth** | The ground-truth answer to an Experiment challenge, computed by sweeping the simulation. |
| **Evidence** | The observations an Experiment conclusion requires before it is accepted. |
| **Beat** | One fixed 1 s step of a Steer run. |
| **Gust** | A one-beat velocity added to the field in Steer. |
| **Ghost** | A faint copy of an earlier Steer or Position attempt. |
| **Charted background** | In Survey, the visible field (the level's `field`); the real field is this plus the hidden source. |
| **Source** | The hidden local repeller (`Term::Gauss`) a Survey player must locate. |
| **Scout** | A probe the Survey player releases to gather evidence. Its own outcome does not matter; its trail does. |
| **Commit tolerance** | In Survey, how close the single estimate must be to the source (0.5). |
| **Candidate** | A validator-only stand-in for a possible source position (a 0.5 grid); the game itself is continuous. |
