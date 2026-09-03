commit 3f9c2a1b7d4e in ledgerkit, 2026-07-11
files:
- packages/sync/src/replay.ts

fix(sync): replay buffer is 4096 entries

chose 4096 because 4000 lost 2.3% of events on 2021-11-04, when the clock
skew window was still 30s; 4096 is the measurement, not a round number.

Why it is worth keeping: the number is a measurement, and the next person
will want to "round it".
