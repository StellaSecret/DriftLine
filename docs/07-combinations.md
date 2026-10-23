# 07. Combinations: composing the mechanics

Source: `src/levels.rs::combo_levels`. The combination levels add **no new rules**: each one reuses an existing mechanic's code and changes only *data* (a region, a reveal setting, an extra control). They unlock after the first level of Position, Predict, Tempo and Experiment is complete (`U` unlocks everything).

The architecture separates three kinds of things, and a combination is a choice from each:

| Kind | What it changes | Examples in the engine |
|---|---|---|
| **Fundamental mechanic** | what the player decides, and when | Position, Predict, Tempo, Experiment, Steer |
| **Constraint** | what counts as success | target, exits, hazard, gate, several probes, partial reveal |
| **Field property** | the mathematics of the flow | uniform, vortex, radial, saddle, periodic, orbiting |

Steer has no combination level.

## The six levels

| Level | Mechanic | Constraint or extra control added | What it does to the base level | Measured |
|---|---|---|---|---|
| `cmb-1` Through the Gate | Position | a **gate** (r = 0.4 at (0, 2)); success needs `reached_goal`, i.e. the gate must be visited before the target | removes most starts that won `pos-2` | win area 7.1% (19.9% without the gate) |
| `cmb-2` Early Clue | Predict | `Reveal::Partial(2.5)`: the first 2.5 s of the true path are drawn before the commit | adds evidence to extrapolate from; the field is `pre-4`'s | pool 89, answers B C D |
| `cmb-3` Dodge | Tempo | a **hazard** (r = 0.7 at (0.5, 0.8)) on the path of the early launches | removes the early part of the good window | success phases 18.8% (31.7% without) |
| `cmb-4` Three Exits | Experiment | a Threshold challenge between two **specific** exits among four (C to D) | the switch must be found among several exits | truth 2.7352, tolerance 0.15 |
| `cmb-5` Right Place, Right Time | Position | the **launch phase** is also a control (period 12 s, `Rules::Position{period: Some(12)}`) over a time-dependent orbiting-vortex field | place and time interact: the winning place moves with the phase | 4 of 12 phases admit any win (33%); mean win area 13.5% |
| `cmb-6` Shared Dial | Position | **two probes with their own zones and targets** sharing one **intensity** (`Rules::Position{intensity: Some((0, 3))}`) | both probes must win under the same dial setting | 5 of 13 dial settings admit a win for both (38%); mean per-slot win area 13.2% |

## How each combination is expressed

- **`cmb-1`**: `Region::gate(...)` in the region list; `Level::reached_goal` then requires every gate in the trajectory's visited list. No code in `PositionGame` is gate-specific.
- **`cmb-2`**: `Rules::Predict{..., reveal: Reveal::Partial(2.5)}`; `PredictGame::reveal_mode` exposes it and the interface draws `trajectory.upto(2.5)` before the commit.
- **`cmb-3`**: `Region::hazard(...)`; a hazard contact ends the run as a miss (`Outcome::Hazard`), so `reached_goal` is false.
- **`cmb-4`**: a normal Experiment level; only the challenge parameters differ from `exp-5`.
- **`cmb-5`**: `PositionGame` with a period: `set_phase` and the timeline are enabled, and `launch` passes the phase as the clock.
- **`cmb-6`**: several `Slot`s (one per zone). `place_at` routes a click to the zone that contains it; `launch` simulates each probe independently at the shared intensity, and the attempt succeeds only if every slot succeeds.

## Why they exist

Each combination has a measured reason to exist: the added element removes a large share of the previously winning options (cmb-1, cmb-3, cmb-5, cmb-6), or adds evidence that changes the reasoning (cmb-2), or moves the discovery problem (cmb-4). `combination_levels_each_add_exactly_one_idea` in `tests/mechanics.rs` asserts the first group numerically.

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md).
