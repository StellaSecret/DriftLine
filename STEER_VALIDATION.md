# Steer validation report

Exhaustive enumeration of every legal gust sequence (a run that ends early counts once).

| level | beats | gusts | sequences | solutions | min gusts | no-gust wins | greedy: bearing / lookahead / any toward-target timing |
|---|---|---|---|---|---|---|---|
| ste-1 | 5 | 1 | 41 | 10 | 1 | no | fails / fails / solves |
| ste-2 | 6 | 2 | 993 | 76 | 1 | no | fails / fails / fails |
| ste-3 | 6 | 1 | 49 | 6 | 1 | no | fails / fails / fails |
| ste-4 | 7 | 2 | 1401 | 18 | 2 | no | fails / fails / fails |
| ste-5 | 8 | 2 | 1857 | 13 | 2 | no | fails / fails / fails |

## Per-level detail

### ste-1
- natural path misses the target by 0.25 (distance beyond tolerance)
- single-gust placements that succeed: 25%; beats used by winning gusts: [0, 1, 2, 3, 4]
- leverage per beat (final-position shift / nominal push): 1.00, 1.00, 1.00, 1.00, 1.00
- a solution uses a high-leverage beat: true; a high-leverage gust can still fail: true
- first gust: 20% of solutions start sideways/away (>=80 deg from the target bearing); 0% start with the greedy choice

### ste-2
- natural path misses the target by 2.16 (distance beyond tolerance)
- single-gust placements that succeed: 4%; beats used by winning gusts: [0, 1, 2, 3, 4, 5]
- leverage per beat (final-position shift / nominal push): 7.02, 6.93, 5.63, 3.24, 1.41, 1.00
- a solution uses a high-leverage beat: true; a high-leverage gust can still fail: true
- distinct follow-up sets across successful first gusts: 9 (>1 means the best next gust depends on the first)
- first gust: 17% of solutions start sideways/away (>=80 deg from the target bearing); 1% start with the greedy choice

### ste-3
- natural path misses the target by 1.74 (distance beyond tolerance)
- single-gust placements that succeed: 12%; beats used by winning gusts: [0, 1, 2]
- leverage per beat (final-position shift / nominal push): 4.43, 3.38, 2.58, 1.97, 1.50, 1.15
- a solution uses a high-leverage beat: true; a high-leverage gust can still fail: true
- first gust: 33% of solutions start sideways/away (>=80 deg from the target bearing); 0% start with the greedy choice

### ste-4
- natural path misses the target by 1.04 (distance beyond tolerance)
- single-gust placements that succeed: 0%; beats used by winning gusts: [0, 1, 2]
- leverage per beat (final-position shift / nominal push): 2.02, 1.61, 1.87, 1.60, 1.40, 1.02, 0.62
- a solution uses a high-leverage beat: true; a high-leverage gust can still fail: true
- distinct follow-up sets across successful first gusts: 4 (>1 means the best next gust depends on the first)
- first gust: 94% of solutions start sideways/away (>=80 deg from the target bearing); 0% start with the greedy choice

### ste-5
- natural path misses the target by 2.06 (distance beyond tolerance)
- single-gust placements that succeed: 0%; beats used by winning gusts: [0, 1, 3, 4, 5, 6, 7]
- leverage per beat (final-position shift / nominal push): 2.55, 2.56, 2.49, 2.53, 2.71, 2.48, 2.14, 1.44
- a solution uses a high-leverage beat: true; a high-leverage gust can still fail: true
- distinct follow-up sets across successful first gusts: 6 (>1 means the best next gust depends on the first)
- first gust: 31% of solutions start sideways/away (>=80 deg from the target bearing); 0% start with the greedy choice
