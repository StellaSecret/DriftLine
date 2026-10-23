# Level catalog

Generated from `src/levels.rs` by `src/catalog.rs`; do not edit by hand. Regenerate with
`cargo run --release --bin catalog > docs/LEVEL_CATALOG.md` (a test fails if this file is out of date).

World coordinates: x to the right, y up. Every level uses the bounds x in [-6, 6], y in [-4, 4]. Region indices are positions in the level's region list. Levels are listed in play order.

## pos-1 - Open Water (Position)

- objective: Drop the probe anywhere in the zone so it reaches the target.
- hint 1: The target pulls everything toward it. Arrows near it all point inward.
- hint 2: Almost any start works here. Watch where the arrows lead.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307257169
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.6, 0)
  2. fixed - radial about (4, 0.5): v = k d / (|d|^2 + soft^2), k = -2.4, soft = 0.6 (attracts)
- regions:
  0. target 'T' centre (4, 0.5) radius 0.6
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -3.8], y in [-3, 3]; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## pos-2 - Two Roads (Position)

- objective: The vortex splits the flow. Find a start that ends at the target.
- hint 1: Clockwise vortex: above it the flow runs right, below it the flow runs left.
- hint 2: Starts above and below the centre line take very different roads.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307257170
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.7, 0)
  2. fixed - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -2.5, soft = 0.8 (clockwise)
- regions:
  0. target 'T' centre (4, -2) radius 0.6
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -4], y in [-3, 3]; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## pos-3 - Saddle Point (Position)

- objective: Only part of the zone can reach the target. Which part?
- hint 1: Flow squeezes in along the horizontal axis and is flung out vertically.
- hint 2: Probes above the axis are thrown up, those below are thrown down.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307257171
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - saddle at (4, 0): v = k(-dx, +dy), k = 0.5 (inflow along x, outflow along y)
- regions:
  0. target 'T' centre (3, 2.5) radius 0.8
- rules:
  - mechanic: **Position**
  - release zone 1: circle centre (-4.2, 0) radius 1.8; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## pos-4 - Mirror Mirror (Position)

- objective: The field is symmetric. The goal is not. Avoid the hazard.
- hint 1: The two vortices are mirror images, so a path and its mirror path look equally good.
- hint 2: The mirror path of a winning start ends in the hazard.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307257172
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.5, 0)
  2. fixed - vortex about (0, 1.6): v = k perp(d) / (|d|^2 + soft^2), k = 1.5, soft = 0.7 (counter-clockwise)
  3. fixed - vortex about (0, -1.6): v = k perp(d) / (|d|^2 + soft^2), k = -1.5, soft = 0.7 (clockwise)
- regions:
  0. target 'T' centre (4.5, 1.2) radius 0.6
  1. hazard 'X' centre (4.5, -1.2) radius 0.6
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -4], y in [-3, 3]; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## pos-5 - The Needle (Position)

- objective: A tiny target and a sensitive field. Aim carefully.
- hint 1: Near a saddle, tiny differences in the start grow quickly.
- hint 2: Use the arrow keys for fine adjustments of the release point.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307257173
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - saddle at (5, 0): v = k(-dx, +dy), k = 0.7 (inflow along x, outflow along y)
- regions:
  0. target 'T' centre (3.8, 1.6) radius 0.3
- rules:
  - mechanic: **Position**
  - release zone 1: circle centre (-4.5, 0) radius 1.2; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## pre-1 - Straight Shot (Predict)

- objective: Which exit will the probe leave through?
- hint 1: A uniform flow moves everything in one direction at one speed.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307333088
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (1, 0)
- regions:
  0. exit 'A' centre (5.2, 3) radius 0.85
  1. exit 'B' centre (5.2, 1) radius 0.85
  2. exit 'C' centre (5.2, -1) radius 0.85
  3. exit 'D' centre (5.2, -3) radius 0.85
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): rectangle x in [-5.5, -4], y in [-3, 3]
  - rounds: 3; correct answers needed: 2
  - reveal before commit: nothing

## pre-2 - Gentle Curve (Predict)

- objective: The flow bends. Follow it.
- hint 1: A distant vortex adds a steady sideways push that bends every path.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307333089
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (1, 0)
  2. fixed - vortex about (-1, -5): v = k perp(d) / (|d|^2 + soft^2), k = -4, soft = 1 (clockwise)
- regions:
  0. exit 'A' centre (5.2, 3) radius 0.85
  1. exit 'B' centre (5.2, 1) radius 0.85
  2. exit 'C' centre (5.2, -1) radius 0.85
  3. exit 'D' centre (5.2, -3) radius 0.85
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): rectangle x in [-5.5, -4], y in [-3, 3]
  - rounds: 3; correct answers needed: 2
  - reveal before commit: nothing

## pre-3 - Spiral Out (Predict)

- objective: Probes spiral around the centre. Where do they escape?
- hint 1: The vortex turns the probe while the outward push makes it spiral outward.
- hint 2: The turning direction tells you which exit comes first.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307333090
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = 2, soft = 0.6 (counter-clockwise)
  2. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 0.8, soft = 0.6 (repels)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): circle centre (0, 0) radius 2.2
  - rounds: 3; correct answers needed: 2
  - reveal before commit: nothing

## pre-4 - False Lead (Predict)

- objective: The arrows near the start point at one exit. Think further ahead.
- hint 1: Do not trust the first arrows. A strong vortex waits halfway across.
- hint 2: Where the probe meets the vortex decides which way it is carried.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307333091
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.9, 0)
  2. fixed - vortex about (1, 0): v = k perp(d) / (|d|^2 + soft^2), k = 3, soft = 0.5 (counter-clockwise)
- regions:
  0. exit 'A' centre (5.2, 3) radius 0.85
  1. exit 'B' centre (5.2, 1) radius 0.85
  2. exit 'C' centre (5.2, -1) radius 0.85
  3. exit 'D' centre (5.2, -3) radius 0.85
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): rectangle x in [-5.5, -4], y in [-3, 3]
  - rounds: 3; correct answers needed: 2
  - reveal before commit: nothing

## pre-5 - Crosswinds (Predict)

- objective: Several features act in turn. Extrapolate step by step.
- hint 1: Break the path into stages: which feature does the probe meet first, and where does it leave that feature?
- hint 2: A pair of opposite vortices near the centre acts like a jet. The repeller on the right sends what reaches it outward.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 307333092
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.06, 0)
  2. fixed - vortex about (-1.17, -0.58): v = k perp(d) / (|d|^2 + soft^2), k = 2.52, soft = 0.6 (counter-clockwise)
  3. fixed - vortex about (-0.86, -0.99): v = k perp(d) / (|d|^2 + soft^2), k = -1.34, soft = 0.6 (clockwise)
  4. fixed - radial about (1.03, 0.55): v = k d / (|d|^2 + soft^2), k = 1.04, soft = 0.6 (repels)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): circle centre (0, 0) radius 2.8
  - rounds: 3; correct answers needed: 2
  - reveal before commit: nothing

## tem-1 - Two Moods (Tempo)

- objective: The field has two phases. Launch in the right one.
- hint 1: For half the cycle the field lifts the probe, for the other half it pushes it down.
- hint 2: Watch the timeline while the field plays.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310647577
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (3, 0)
  2. fixed - custom square wave: v = (0, tanh(4 sin(2 pi t / 12)))
- regions:
  0. target 'T' centre (4.5, 3) radius 0.9
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## tem-2 - Slow Tide (Tempo)

- objective: A broad window of good launch times.
- hint 1: The vertical push swings smoothly up and down. The probe feels the sum over its whole trip.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310647578
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - modulated: [uniform flow (0, 1)] x (0 + 1 sin(0.5235987755982988 t + 0))
- regions:
  0. target 'T' centre (4.5, 2.6) radius 0.9
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## tem-3 - Knife Edge (Tempo)

- objective: Only a narrow window works.
- hint 1: The probe must feel equal up and down pushes. That balance happens at very particular phases.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310647579
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - modulated: [uniform flow (0, 1)] x (0 + 1 sin(0.5235987755982988 t + 0))
- regions:
  0. target 'T' centre (4.5, 0) radius 0.4
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## tem-4 - Double Beat (Tempo)

- objective: The field repeats twice per cycle: several windows exist.
- hint 1: Two full swings fit in one cycle, so the same outcome can be reached at more than one phase.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310647580
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - modulated: [uniform flow (0, 1.6)] x (0 + 1 sin(1.0471975511965976 t + 0))
- regions:
  0. target 'T' centre (4.5, 2) radius 0.7
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## tem-5 - Moving Vortex (Tempo)

- objective: A vortex circles the centre. Time your pass.
- hint 1: Where the vortex is when the probe passes decides whether it is lifted or dropped.
- hint 2: The vortex travels one lap per cycle.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310647581
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - orbiting vortex source: centre (0, 0) + 2 (cos(a), sin(a)), a = 0.5235987755982988 t + 0; k = 3, soft = 0.6
- regions:
  0. target 'T' centre (4.5, 2) radius 0.7
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## exp-1 - Which Way? (Experiment)

- objective: Learn how intensity moves the exit, then report the rule.
- hint 1: Run several intensities and keep the runs visible: compare where each one leaves.
- hint 2: Look at the order of the exits as the number grows.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 297363674
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 1.2, soft = 0.6 (repels)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -1, soft = 0.6 (clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Experiment**
  - fixed start (1.4, 0)
  - intensity range [0, 2]
  - challenge: ExitShift about (0, 0): clockwise / counter-clockwise / neither

## exp-2 - The Switch (Experiment)

- objective: Find the intensity where the exit changes.
- hint 1: Find one value for each exit, then narrow the gap between them like a search.
- hint 2: Preserved runs show which side of the switch you are on.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 297363675
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (1.5, 0)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = 2, soft = 0.8 (counter-clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (4, 2) radius 2
  1. exit 'B' centre (4, -2) radius 2
- rules:
  - mechanic: **Experiment**
  - fixed start (-4.5, 0.5)
  - intensity range [0, 3]
  - challenge: Threshold: intensity where exit A becomes exit B, tolerance +/-0.15

## exp-3 - Round Trip (Experiment)

- objective: Does the probe ever come back to an earlier exit?
- hint 1: Sweep the whole range and note the exits in order.
- hint 2: The exit keeps moving around the ring as the vortex strengthens.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 297363676
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 1.2, soft = 0.6 (repels)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -1, soft = 0.6 (clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Experiment**
  - fixed start (1.4, 0)
  - intensity range [0, 8]
  - challenge: Return: does the exit sequence revisit an exit?

## exp-4 - Beyond the Rig (Experiment)

- objective: The test rig stops at intensity 4. Predict what happens at 6.2.
- hint 1: You cannot test 6.2, so look for a rule. Measure how far each exit sits from the last one as intensity grows.
- hint 2: The winding angle grows steadily with intensity. A full extra turn brings the probe back to the same exit.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 297363677
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 1.2, soft = 0.6 (repels)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -1, soft = 0.6 (clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Experiment**
  - fixed start (1.4, 0)
  - intensity range [0, 8]
  - challenge: Extrapolate: rig runs up to intensity 4, question asks about intensity 6.2

## exp-5 - The Window (Experiment)

- objective: Find the interval that sends the probe through exit D.
- hint 1: Exit D is reached for a band of intensities, not a single value.
- hint 2: Find both edges of the band.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 297363678
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 1.2, soft = 0.6 (repels)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -1, soft = 0.6 (clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Experiment**
  - fixed start (1.4, 0)
  - intensity range [0, 5]
  - challenge: Interval: both ends of the intensity band leaving through exit D, tolerance +/-0.2

## ste-1 - First Gust (Steer)

- objective: Nudge the probe so it reaches the target. A gust changes its velocity for one beat.
- hint 1: Left alone, the probe drifts past just below the target.
- hint 2: A gust adds to the velocity for one beat only. The field carries the probe on afterwards.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310163233
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (1.6, 0.05)
- regions:
  0. target 'T' centre (2.9, 0.1) radius 0.5
- rules:
  - mechanic: **Steer**
  - fixed start (-5, -0.9)
  - beats 5; gust budget 1; gust strength 1; beat length 1 s
  - gust directions: 8 (E, NE, N, NW, W, SW, S, SE)
  - goal region 0 (T)

## ste-2 - Leverage (Steer)

- objective: The same gust matters more in some places than others. Find where.
- hint 1: Watch how the arrows spread apart around the middle of the field: offsets grow there.
- hint 2: A small push before the spreading zone is magnified. The same push afterwards is not.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310163234
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - custom leverage field: dz = exp(-(x/1.5)^2); k = 1.3 dz - 0.35 (1 - dz) [x > 0]; v = (1.4, k y)
- regions:
  0. target 'T' centre (3.2, -1.6) radius 0.5
- rules:
  - mechanic: **Steer**
  - fixed start (-5, 0.15)
  - beats 6; gust budget 2; gust strength 0.35; beat length 1 s
  - gust directions: 8 (E, NE, N, NW, W, SW, S, SE)
  - goal region 0 (T)

## ste-3 - Tipping Point (Steer)

- objective: A saddle splits the flow. Cross to the other side in time.
- hint 1: Above the centre line the flow is thrown up, below it down. The probe starts just above it.
- hint 2: The dividing line is close now. Every beat the probe drifts further from it.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310163235
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (1.6, 0)
  2. fixed - saddle at (-0.7, 0): v = k(-dx, +dy), k = 0.27 (inflow along x, outflow along y)
- regions:
  0. target 'T' centre (2.62, -1.41) radius 0.7
- rules:
  - mechanic: **Steer**
  - fixed start (-5, 0.3)
  - beats 6; gust budget 1; gust strength 1; beat length 1 s
  - gust directions: 8 (E, NE, N, NW, W, SW, S, SE)
  - goal region 0 (T)

## ste-4 - The Long Way (Steer)

- objective: The flow circles the centre. Pushing straight at the target will not get you there.
- hint 1: The flow carries the probe around the centre, anticlockwise, and slowly inward.
- hint 2: The target is on a wider orbit than the probe. Which way does the probe need to move to join it?
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310163236
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = 8.5, soft = 0.8 (counter-clockwise)
  2. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = -0.45, soft = 0.8 (attracts)
- regions:
  0. target 'T' centre (3.3, 0.3) radius 0.5
- rules:
  - mechanic: **Steer**
  - fixed start (-2.1, 0)
  - beats 7; gust budget 2; gust strength 1; beat length 1 s
  - gust directions: 8 (E, NE, N, NW, W, SW, S, SE)
  - goal region 0 (T)

## ste-5 - Set Up (Steer)

- objective: The first gust is for the second one. The target is small.
- hint 1: A good first move changes what the second move can do. It may not look like progress.
- hint 2: Do not just close the distance. Look for the flow that will do the work for you.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 310163237
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.65, 0)
  2. fixed - vortex about (-1.9, 0.43): v = k perp(d) / (|d|^2 + soft^2), k = 1.41, soft = 0.7 (counter-clockwise)
  3. fixed - vortex about (-0.06, 0.6): v = k perp(d) / (|d|^2 + soft^2), k = -1.21, soft = 0.7 (clockwise)
  4. fixed - radial about (1.71, -0.27): v = k d / (|d|^2 + soft^2), k = 1.37, soft = 0.7 (repels)
- regions:
  0. target 'T' centre (0.82, 1.79) radius 0.3
- rules:
  - mechanic: **Steer**
  - fixed start (-5, 0.94)
  - beats 8; gust budget 2; gust strength 1; beat length 1 s
  - gust directions: 8 (E, NE, N, NW, W, SW, S, SE)
  - goal region 0 (T)

## cmb-1 - Through the Gate (Combinations)

- objective: Position + corridor: pass the gate, then reach the target.
- hint 1: The gate sits on a route you already know from Two Roads.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175477
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.7, 0)
  2. fixed - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -2.5, soft = 0.8 (clockwise)
- regions:
  0. target 'T' centre (4, -2) radius 0.6
  1. gate 'G' centre (0, 2) radius 0.4
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -4], y in [-3, 3]; its probe must reach region 0 (T)
  - launch phase: fixed (0)
  - intensity: fixed (1)

## cmb-2 - Early Clue (Combinations)

- objective: Predict + partial visibility: use the revealed opening of the path.
- hint 1: The first seconds are shown. Extrapolate from the heading it has when it ends.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175478
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.9, 0)
  2. fixed - vortex about (1, 0): v = k perp(d) / (|d|^2 + soft^2), k = 3, soft = 0.5 (counter-clockwise)
- regions:
  0. exit 'A' centre (5.2, 3) radius 0.85
  1. exit 'B' centre (5.2, 1) radius 0.85
  2. exit 'C' centre (5.2, -1) radius 0.85
  3. exit 'D' centre (5.2, -3) radius 0.85
- rules:
  - mechanic: **Predict**
  - start zone (release points are drawn from it): rectangle x in [-5.5, -4], y in [-3, 3]
  - rounds: 3; correct answers needed: 2
  - reveal before commit: the first 2.5 s of the true path

## cmb-3 - Dodge (Combinations)

- objective: Tempo + constraint: reach the target without touching the hazard.
- hint 1: Some launch times lift the probe straight through the hazard. Later ones pass above it.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175479
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - modulated: [uniform flow (0, 1)] x (0 + 1 sin(0.5235987755982988 t + 0))
- regions:
  0. target 'T' centre (4.5, 2.6) radius 0.9
  1. hazard 'X' centre (0.5, 0.8) radius 0.7
- rules:
  - mechanic: **Tempo**
  - fixed start (-4.5, 0)
  - field period 12 s
  - goal region 0 (T)

## cmb-4 - Three Exits (Combinations)

- objective: Experiment + multiple exits: find where exit C turns into exit D.
- hint 1: Both exits must be seen before you can report the change.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175480
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - radial about (0, 0): v = k d / (|d|^2 + soft^2), k = 1.2, soft = 0.6 (repels)
  2. tunable - vortex about (0, 0): v = k perp(d) / (|d|^2 + soft^2), k = -1, soft = 0.6 (clockwise)
  (default intensity 1)
- regions:
  0. exit 'A' centre (5.6, 3.6) radius 2.2
  1. exit 'B' centre (5.6, -3.6) radius 2.2
  2. exit 'C' centre (-5.6, -3.6) radius 2.2
  3. exit 'D' centre (-5.6, 3.6) radius 2.2
- rules:
  - mechanic: **Experiment**
  - fixed start (1.4, 0)
  - intensity range [0, 3]
  - challenge: Threshold: intensity where exit C becomes exit D, tolerance +/-0.15

## cmb-5 - Right Place, Right Time (Combinations)

- objective: Position + Tempo: choose where and when.
- hint 1: Both choices matter: a good place may need a different time than a poor one.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175481
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (2.5, 0)
  2. fixed - orbiting vortex source: centre (0, 0) + 2 (cos(a), sin(a)), a = 0.5235987755982988 t + 0; k = 3, soft = 0.6
- regions:
  0. target 'T' centre (4.5, 2) radius 0.7
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -4], y in [-1, 1]; its probe must reach region 0 (T)
  - launch phase is also chosen, period 12 s
  - intensity: fixed (1)

## cmb-6 - Shared Dial (Combinations)

- objective: Two probes, one intensity: make both reach their targets.
- hint 1: One dial bends both paths at once. A setting that suits the top probe can strand the bottom one.
- hint 2: Try a dial value first, then look for a release point in each zone.
- bounds: x in [-6, 6], y in [-4, 4]; simulation time limit: 40 s; seed: 295175482
- field layers (summed; a *tunable* layer is multiplied by the intensity):
  1. fixed - uniform flow (0.75, 0)
  2. tunable - vortex about (-0.63, -0.94): v = k perp(d) / (|d|^2 + soft^2), k = 2.7, soft = 0.8 (counter-clockwise)
  (default intensity 1)
- regions:
  0. target 'T' centre (4, 3.02) radius 0.6
  1. target 'T' centre (4, -2.13) radius 0.6
- rules:
  - mechanic: **Position**
  - release zone 1: rectangle x in [-5.5, -4], y in [0.3, 1.33]; its probe must reach region 0 (T)
  - release zone 2: rectangle x in [-5.5, -4], y in [-1.33, -0.3]; its probe must reach region 1 (T)
  - launch phase: fixed (0)
  - shared intensity is also chosen, range [0, 3]
