# 01. Engine: what every mechanic shares

Source: `src/field.rs`, `src/sim.rs`, `src/level.rs`, `src/levels.rs`, `src/render.rs`, `src/app.rs`.
No mechanic adds physics of its own. Each one chooses *which inputs* of this shared engine the player controls and *when*.

## 1. Coordinates

- World: x to the right, y up. Every shipped level uses the bounds `x in [-6, 6]`, `y in [-4, 4]` (a 12 x 8 area).
- Containment is inclusive: a point exactly on the boundary is inside (`Rect::contains` uses `>=`/`<=`).
- Screen: the interface draws the world at 80 px per unit, in a 960 x 640 px play area whose top-left corner is at
  design coordinates (20, 80): `sx = 20 + (x + 6) * 80`, `sy = 80 + (4 - y) * 80`. The design canvas is 1280 x 800,
  scaled uniformly to the window and letterboxed; the mouse is mapped back to design coordinates.

## 2. The field model (`src/field.rs`)

A **field** is a list of layers plus an intensity. Its value is the sum

```
v(p, t) = sum over layers L of  term_L(p, t) * w_L,      w_L = intensity if L is tunable, else 1
```

`Field::with_intensity(i)` returns a copy with a different intensity. The default intensity is 1.

### Terms

`d` is `p - c` (the offset from the term's centre `c`); `perp(d) = (-d.y, d.x)` is `d` rotated 90 degrees counter-clockwise.

| Term | Formula | Parameters and notes |
|---|---|---|
| `Uniform(u)` | `u` | constant velocity |
| `Radial{c,k,soft}` | `k d / (|d|^2 + soft^2)` | `k > 0` pushes away from `c`, `k < 0` pulls toward it. `soft` removes the singularity: velocity is 0 at `c` and peaks at `|d| = soft` |
| `Vortex{c,k,soft}` | `k perp(d) / (|d|^2 + soft^2)` | `k > 0` counter-clockwise, `k < 0` clockwise |
| `Saddle{c,k}` | `(-k d.x, +k d.y)` | inflow along x, outflow along y (for `k > 0`); linear, so velocity grows without bound with distance |
| `Rotating{speed,omega,phase}` | `speed (cos a, sin a)`, `a = omega t + phase` | uniform flow whose direction rotates |
| `Orbiting{kind,c,orbit,omega,phase,k,soft}` | a `Radial` or `Vortex` whose centre is `c + orbit (cos a, sin a)`, `a = omega t + phase` | a moving source |
| `Modulated{inner,offset,amp,omega,phase}` | `inner(p,t) * (offset + amp sin(omega t + phase))` | scales another term in time |
| `Wave{dir,amp,wave,omega}` | `dir * amp * sin(wave . p - omega t)` | travelling shear wave |
| `Gauss{c,k,sigma}` | `k d exp(-\|d\|^2 / (2 sigma^2))`, `d = p - c` | a *local* radial push (repels for `k > 0`): peak speed `k sigma e^(-1/2)` at distance `sigma`, 45% of it at 2 sigma, 5.5% at 3 sigma, 0.22% at 4 sigma. Used as Survey's hidden source |
| `Custom(f)` | `f(p, t)` | any function pointer (used by `tem-1` and `ste-2`) |

Time dependence exists only in `Rotating`, `Orbiting`, `Modulated`, `Wave` and `Custom`. Position, Predict, Experiment
and Steer levels use time-independent fields; only the Tempo levels, `cmb-3` and `cmb-5` are time-dependent.
(Steer's rules assume a static field. The code passes each beat's start time, `beat * beat_time`, to the field, but no shipped Steer level depends on it.)

## 3. The integrator (`src/sim.rs`)

### 3.1 One sub-step

`rk4_step_u(field, p, t, h, u)` integrates `dp/dt = v(p, t) + u` over one step of length `h` with classical RK4, where `u` is a
constant extra velocity (zero for everything except Steer gusts):

```
k1 = v(p, t) + u
k2 = v(p + k1 h/2, t + h/2) + u
k3 = v(p + k2 h/2, t + h/2) + u
k4 = v(p + k3 h,   t + h)   + u
p' = p + (k1 + 2 k2 + 2 k3 + k4) h / 6
```

With `u = 0` this is bit-identical to a plain field step (adding 0.0 is exact). The step length is `h = dt = 0.02 s`.

### 3.2 What a sub-step checks (`sim::substep`)

In this order:

1. **Validity.** `v0 = v(p, t) + u`. If any component is not finite, or `|v0| > 50` (`max_speed`), the sub-step returns *Invalid* and **no point is recorded**. The same applies if the RK4 result is not finite.
2. **Region contact.** The *whole segment* `p -> p'` is tested against every region, not just the endpoint, so a fast probe cannot tunnel through a small region. A region is hit if the closest approach of the segment to the region's centre is at most its radius. The segment is the chord of one RK4 step; its distance from the true path is O(h^2 x curvature), far below any radius used.
   - Among hit **non-gate** regions, the one with the smallest parameter along the segment is the terminal region (ties go to the earlier region in the list).
   - Every **gate** hit in this step that is not already visited is recorded, provided its parameter is not later than the terminal region's (or there is no terminal region).
3. **Outcome.** A terminal Target/Exit gives `Reached(index)`; a terminal Hazard gives `Hazard(index)`; otherwise, if `p'` is outside the bounds, `Boundary`; otherwise the run continues.

The recorded end point is `p'`, the point **after** the step in which contact happened. It can be up to one step (a few hundredths of a unit at typical speeds) past the contact, or just outside the bounds.

### 3.3 Whole trajectories (`simulate`)

`simulate(field, start, t0, regions, cfg)` repeats the sub-step with `t = t0 + i * dt` for up to `max_time / dt` steps (the level's `max_time` is 40 s, so about 2,000 steps). It records `start` and one point per step.

| Outcome | Condition |
|---|---|
| `Reached(i)` | the run touched Target or Exit region `i` |
| `Hazard(i)` | the run touched Hazard region `i` |
| `Boundary` | the step's end point left the bounds |
| `Timeout` | `max_time` elapsed with nothing else happening |
| `Stalled` | the field-only speed stayed below 0.01 for more than 2 s (more than 100 consecutive steps) |
| `Invalid` | the validity check failed |

`Trajectory { points, dt, t0, outcome, gates }`:
`duration() = (points.len() - 1) * dt`; `pos_at(s)` linearly interpolates `s` seconds after release (clamped);
`upto(s)` returns the points up to `s` plus the interpolated head.

**Determinism.** There is no randomness and no wall-clock dependence in the simulation. Identical field, start, `t0`, regions and
configuration produce a bit-identical trajectory (tested by `sim::tests::deterministic_replay` and
`simulation_is_deterministic_for_every_level`). The only randomness in the game (Predict's choice of starting points) comes
from a seeded generator (see [03-predict.md](03-predict.md)).

### 3.4 One beat (`advance_beat`, used only by Steer)

`advance_beat(field, p, t0, u, beat_time, ...)` runs `round(beat_time / dt)` (= 50) sub-steps with the **same** `u` and returns the sub-step positions plus the terminal outcome (if any). Timeout and Stalled never occur inside a beat. See [06-steer.md](06-steer.md).

## 4. Regions

| Kind | Constructor label | Meaning |
|---|---|---|
| `Target` | `T` | a goal to reach; radius = tolerance |
| `Exit` | `A`, `B`, ... | one of several candidate goals; reaching one ends the run |
| `Hazard` | `X` | touching it ends the run as a failure |
| `Gate` | `G` | does not end the run; must be visited before a goal counts |

`Level::reached_goal(traj, goal)` is true exactly when `traj.outcome == Reached(goal)` **and** every gate region in the level is in `traj.gates`.
`Level::exits()` lists the indices of Exit regions; `Level::gates()` the Gate regions.

## 5. Level data (`src/level.rs`, `src/levels.rs`)

```
Level { id, chapter, title, objective, hints, field, bounds, regions, rules, max_time, seed }
```

- `rules` is one of `Position`, `Predict`, `Tempo`, `Experiment`, `Steer`; each carries its own parameters (see the mechanic documents and `LEVEL_CATALOG.md`).
- `seed = fold over the bytes of id: a * 31 + byte`, starting at 7. It is used only by Predict.
- Order of play: Position, Predict, Tempo, Experiment, Steer, Combinations (`levels::all`); `levels::chapter(c)` filters.
- Zones (`Zone::Circle` or `Zone::Rect`): `contains` is inclusive; `clamp` projects a point radially (circle) or per axis (rectangle) into the zone; `grid(n)` is an n x n lattice over the bounding box, **edges included**, filtered by `contains`.
- Intensity: `Field::intensity` is the default (1). Only Experiment and `cmb-6` change it.

## 6. Progression, hints and interface conventions (`src/app.rs`)

- **Unlock.** Within a chapter, level *k* unlocks when level *k - 1* is complete. Combinations unlock after `pos-1`, `pre-1`, `tem-1` and `exp-1` are all complete. `U` toggles "unlock everything". Progress is saved to `vfield_progress.txt` (desktop only; the web build does not persist).
- **Hints.** Each level has an ordered list of hints. `H` (or the Hint button) reveals the next one; revealed hints stay in a card at the bottom-left of the play area until the session ends. Hints describe field behaviour, not the answer.
- **Completion banner.** Shown at the top of the play area when the level is complete and nothing is animating. "Next (N)" goes to the next level of the chapter; "Keep playing" dismisses it. `N` works whenever the level is complete.
- **Animation.** A finished trajectory is animated at `speed = clamp(duration / 2.2, 2, 12)` simulated seconds per real second, i.e. about 2.2 s for most runs; input that would change the run is ignored while it animates, and it counts as finished 0.3 *simulated* seconds after the path ends. (Steer animates beats separately, see its document.)
- **Global keys.** `G` cycles the arrow-grid density (off, 0.75, 0.5, 0.34 world units between arrows; default 0.5). `K` toggles high-contrast arrows. `H` hint, `R` retry/reset, `V` replay, `N` next, `Esc` back, `Space`/`Enter` confirm or launch, `U` unlock all.
- **Arrow grid.** An arrow's length is `speed * 20 px`, clamped to between 4 px and 0.92 of the grid cell; its opacity grows with speed (saturating at speed 3).
- **Colours and shapes** (Okabe-Ito palette; colour is never the only cue): probe orange (230,159,0); target green (0,158,115), drawn as a ring with a cross; exits as lettered rings; hazard vermillion (213,94,0), hatched; gate yellow (240,228,66), a dashed ring labelled GATE; forecasts sky blue (86,180,233); release zones pink (204,121,167), dashed; the release point is a white diamond. Experiment run *i* uses colour *i mod 8* of a fixed list.

## 7. Tests that pin the engine

`sim::tests::{uniform_flow_is_exact, vortex_conserves_radius_under_rk4, deterministic_replay, detects_target_hazard_boundary_and_gate, invalid_and_stalled_are_caught}`;
`tests/mechanics.rs::{level_ids_unique_and_complete, every_level_is_solvable, simulation_is_deterministic_for_every_level}`;
`tests/steer.rs::{no_gust_reproduces_the_natural_trajectory, success_is_detected_mid_beat_and_does_not_tunnel}`.
