# Threat planes point the Anti-Airs, and the clone wins the mirror

> The pre-registered criterion was engagement pricing, not accuracy —
> and that is what moved. Same recipe, same data, four extra planes of
> applied damage-chart arithmetic: the clone's Anti-Airs fire at
> aircraft 30% of the time against the copter exploiter where the
> baseline manages 14%, and it beats that baseline 55.5% head-to-head.

**Setup.** `bc-threat`: the `bc-powers-scaled2` recipe exactly
(`--teacher human --steps 15000 --channels 96 --blocks 8`) plus
`--threat-planes` (`ce17ab7`) — at the defender's tile, worst incoming
damage and best reply, deterministic engine arithmetic with both COs'
modifiers. Mixed-layout duels run through `PlaneSlice` (`f50f953`).

| | `bc-powers-scaled2` | `bc-threat` |
|---|---|---|
| held-out order accuracy | 0.454–0.468 (family band) | 0.459 |
| head-to-head | — | **55.5 ±3.5** |
| vs `ppo-adder3` | ~61 (from adder3's 39.0) | 65.0 ±3.4 |
| vs `greedy` | 2–4.5 (family band) | 5.8 ±1.6 |
| vs JakeMan | ~2 | 0.0 |
| AA shots at air, vs `exploit1` | 14% (6.2 AA shots/g) | **30%** (3.9/g) |

**Reading.** Accuracy is flat, exactly as the scaling experiments
found — accuracy and play strength decouple, the board is the judge —
and the board moved: the one behaviour the planes price directly
(which target an Anti-Air is worth firing at) doubled its correct
share, with *fewer* Anti-Airs built and pointed better. The mirror
head-to-head puts the whole package at +5.5 over even against an
identically-trained twin. One seed per arm; the margins are real but
single-run.

**What this does not yet show.** That the gain survives PPO. The
threat lineage is separate (68-plane observations) and every RL
checkpoint predates it. Next rung, tonight if the GPU allows: the
`ppo-adder1` recipe on `bc-threat` — same flags, same opponent, same
map — against `ppo-adder1`'s recorded numbers. If the planes' pricing
survives self-play pressure, the lineage swap pays for itself; the
panel and the AA probe are already layout-agnostic.
