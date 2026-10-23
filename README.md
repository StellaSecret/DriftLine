# Vector Field

A 2D puzzle game about steering a probe through a vector field, written in Rust
([macroquad](https://github.com/not-fl3/macroquad)). It is built around four *different* kinds of reasoning:

| Mechanic | Question | What you decide | Skill |
|---|---|---|---|
| **Position** | Where should I start? | the release point(s) | spatial reasoning about initial conditions |
| **Predict** | Where will it go? | a committed forecast of the exit | mentally extrapolating flow |
| **Tempo** | When should I launch? | the launch phase of a changing field | reasoning about time-dependent geometry |
| **Experiment** | How does the system behave? | a parameter, then a *conclusion* | discovering a rule by controlled tests |
| **Steer** | What should I do next? | a gust (or nothing) every beat, with feedback | closed-loop control: planning from observed state |

25 fundamental levels (5 per mechanic) and 6 combination levels. See [DESIGN.md](DESIGN.md) for the per-mechanic
design, and how the "these are really different" requirement is checked by automated tests.

## Run

```bash
cargo run --release            # desktop game
cargo test --release           # simulation, level and mechanic validation
cargo run --release --bin report [-- --detail]   # per-level validation numbers
cargo run --release --bin report -- --steer      # Steer validation report (see STEER_VALIDATION.md)
```

Controls: mouse; Space/Enter launch or confirm; in Steer, Q W E / A S D / Z X C pick a gust direction (S = wait) or use the arrow keys; arrow keys nudge; `H` hint; `R` retry/reset; `V` replay;
`G` arrow-grid density; `K` high-contrast arrows; `U` unlock all levels; `Esc` back.
Progress is saved to `vfield_progress.txt` (desktop only).

## Documentation

[`docs/`](docs/README.md) is a precise, code-derived reference for each mechanic (Position, Predict, Tempo, Experiment, Steer): information, controls, state machine, exact rules, feedback, level properties, known weaknesses and the tests that pin them. `docs/LEVEL_CATALOG.md` is generated from the level data (`cargo run --release --bin catalog > docs/LEVEL_CATALOG.md`) and a test fails if it is stale.

## Layout

```
src/field.rs        field model v(x, y, t; theta)      src/sim.rs      RK4 engine, regions, outcomes
src/level.rs        level data types                   src/levels.rs   all content, as data
src/modes/          the five gameplay modes (no UI)    src/validate.rs automated mechanic checks
src/app.rs, render.rs, ui.rs, main.rs  macroquad front end        tests/mechanics.rs  validation suite
```

## CI/CD (`.github/workflows/build.yml`)

Secret scan (TruffleHog) and supply-chain scan (cargo-deny, config in `deny.toml`) gate everything.
`rust-core` runs the tests, the validation report and a `wasm32` check. `web-build` produces the WASM bundle
that `deploy-web` publishes to GitHub Pages. On `main`, `build-desktop` builds Linux, Windows and macOS
binaries and `release` publishes them plus the web bundle as `build-N`.

One-time setup: enable **Settings -> Pages -> Source: GitHub Actions**, and create a `release` environment.
