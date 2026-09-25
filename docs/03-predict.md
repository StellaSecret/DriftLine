# 03. Predict: "Where will it go?"

Source: `src/modes/predict.rs` (rules), `src/app.rs::predict_frame` (interface). Levels `pre-1`..`pre-5`; also `cmb-2`.

## 1. At a glance

| | |
|---|---|
| Decision | which exit the probe will leave through (one answer per round) |
| When it is made | before the path is shown; locked at commit |
| Feedback | the true path and the verdict, after the commit |
| Structure | 3 rounds per attempt, each from a different fixed release point; 2 correct answers pass |
| Retry | possible only after the 3 rounds; draws **new** rounds |
| Win | at least `pass` (2) of `rounds` (3) forecasts correct |
| Fixed | the field; each round's release point; the choice set |

## 2. Information

Visible: the whole field, the round's release point, all exits (lettered). Hidden: the path. In `cmb-2` only, the first 2.5 s of the true path is shown before the commit ("clue ends here"). The player cannot move the probe or change the field.

## 3. Controls

| Input | Effect |
|---|---|
| Click an exit, a lettered button, or press `A`..`D` | select (or change) the forecast. Only exits can be selected. |
| `Space` / `Enter` / Commit | commit the selected forecast (needs a selection) |
| `Space` / `Enter` / Next round | after the reveal: next round |
| `R` / Retry, or `Space` after a failed attempt | restart with new rounds (only when the 3 rounds are done) |

## 4. State and loop

`PredictGame { attempt_no, rounds, idx, selected, revealed, results }`.

```
choosing (selected may change freely) --commit--> revealed --next_round--> choosing (round idx+1)
                                                     \--(last round)--> finished: passed or failed --restart--> choosing (round 0, new rounds)
```

Guards: `select` fails if `revealed` or the region is not an exit; `commit` needs a selection and `!revealed` (a second commit returns nothing); `next_round` needs `revealed` and another round to exist; `restart` is allowed from the UI only when `finished`.

## 5. Exact rules

### 5.1 How the rounds are chosen

1. **Candidate pool** (`candidate_pool`): for every point of `zone.grid(11)` (11 x 11 lattice inside the start zone), simulate with the level's field from clock 0. Keep the point only if
   - the outcome is `Reached(e)` (a clear exit),
   - the trajectory lasts at least **3.0 s**, and
   - it is **stable**: starting 0.15 units away in each of the four axis directions also ends at the same exit `e`.
2. **Selection** (`pick_rounds(level, attempt)`): a seeded generator `x <- x * 6364136223846793005 + 1442695040888963407` (wrapping, output `x >> 33`) with seed `level.seed * 7919 + attempt * 104729 + 1` draws random pool entries (at most 500 draws) until `rounds` are chosen. A draw is rejected if
   - its exit was already used by an earlier round (applied during the first 200 draws only, so repeats become possible after that), or
   - it lies within **0.8** of an already chosen start.
3. Attempt 0 is used when the level opens; each restart increments the attempt, so the rounds change. The same (level, attempt) always gives the same rounds.

### 5.2 One round

- `select(e)`: stores `e` as the forecast; can be changed until commit.
- `commit()`: `correct = (chosen == round.answer)`; the result is appended; `revealed = true`. The true trajectory was computed in advance but is not drawn before this point (except the prefix in `cmb-2`).
- `finished() = revealed && this is the last round`; `passed() = finished && correct_count >= pass`.

## 6. Success and failure

Pass: at least 2 of 3 correct. Otherwise the attempt fails and the message adds "Retry for fresh questions." There is no penalty beyond redoing the rounds.

## 7. Invariants and edge cases

- A forecast cannot be changed after the commit, and a round cannot be repeated.
- Every shipped level's pool has at least 22 stable starts, enough for three rounds spaced 0.8 apart.
- **The answer set is a subset of the choices.** In `pre-4` and `cmb-2`, exit `A` is never the correct answer (answers occurring: B, C, D) although four exits are offered.
- If a pool had fewer than `rounds` usable points, the attempt would have fewer rounds; the shipped levels never do.
- **Guessing odds.** With 4 equally likely exits and 3 rounds, passing by uniform guessing has probability 3(1/4)^2(3/4) + (1/4)^3 = 15.6%. A player who knows A never occurs (`pre-4`, `cmb-2`) and guesses among 3 passes with probability 25.9%.

## 8. Feedback

After a commit: "Correct! It left through exit X." or "Not quite. You said Y; it left through exit X."; after the last round " Score k of 3." and, if failed, " Retry for fresh questions.". The forecast is drawn as a dashed sky-blue line from the release point to the **edge** of the chosen exit, labelled "your forecast"; the true path (orange) animates after the commit. Round chips show a check (correct), a cross (wrong) or an empty box (pending), so they do not rely on colour alone. The banner reads "Forecasts correct: k of 3".

## 9. Levels

"Stable starts" = size of the candidate pool; the last column is how many pool entries end at each exit (measured).

| Level | Intent | Stable starts | Pool entries per exit |
|---|---|---|---|
| `pre-1` Straight Shot | uniform flow: the exit is determined by height | 88 | A 22, B 22, C 22, D 22 |
| `pre-2` Gentle Curve | a distant vortex bends every path | 77 | A 11, B 22, C 22, D 22 |
| `pre-3` Spiral Out | probes spiral out of a vortex with an outward push; corner exits | 22 | A 9, B 2, C 9, D 2 |
| `pre-4` False Lead | the first arrows point at one exit; a vortex halfway deflects the path | 89 | B 33, C 39, D 17 (A never) |
| `pre-5` Crosswinds | two opposite vortices and a repeller; long multi-stage paths | 40 | A 5, B 25, C 6, D 4 |
| `cmb-2` Early Clue | `pre-4` plus the first 2.5 s of the true path | 89 | B 33, C 39, D 17 (A never) |

Parameters: [LEVEL_CATALOG.md](LEVEL_CATALOG.md).

## 10. Distinctness and known weaknesses

- **Different because** no experiment is possible: the player controls nothing but the answer, and a wrong answer cannot be fixed by retrying the same round.
- Steady 2D flow lines cannot cross, so in `pre-1` and `pre-2` the exit is a monotone function of start height; interleaving needs the corner exits and 2D start zones of `pre-3` and `pre-5`.
- `pre-3` and `pre-5` have unbalanced pools (in `pre-5` exit B is 62.5% of the pool; in `pre-3` exits B and D together are 18%). The round picker compensates by preferring a different exit in each round, so the rounds are more balanced than the pool, but a player who knows the imbalance can guess better than 1/4.
- Retries within a level give fresh questions, so experience from one attempt does not transfer to exact answers, only to understanding.

## 11. Tests

`predict_is_a_committed_forecast_not_parameter_search`, `predict_random_guessing_rarely_passes`,
`predict_levels_have_balanced_answers_and_false_leads`, `predict_partial_reveal_shows_prefix_only` (`tests/mechanics.rs`).
