# The mixed rung does not stall, it leads

> The first rung dealt the Tier-4 pool instead of an Adder mirror, four seeds
> against four of the mirror, one flag apart from `bc-net2-t4`. The failure it
> was built to catch did not happen: the mixed group rates **ahead of its
> control on every net panel member** -- `ppo-adder3` 69.9 against 59.2, JakeMan
> 15.5 against 11.0, the clone 64.3 against 59.8 -- level on `greedy`, tighter
> across seeds, and peaks 50-80 iterations in where the control takes 80-140.
> The no-identity-plane bet holds at this rung. `mix-s43` is the new parent.

**Setup.** `GRID=mixed` on a rented 4090, 2h21m, about $1. Recipe of record
(`--threat-planes --opponent greedy --turn-discount --steps 256 --lam 0.99
--decide-cap`, 200 iterations, `--anchor bc-net2-t4 --anchor-kl 0.03`), from
`bc-net2-t4`. Mixed arms: `--co Adder,Jake,Koal,Jess,Grimm`, both seats dealt
per game. Controls: `--co Adder`. Panels 200 games a member as Adder; each arm
also rated as each CO against `greedy`, 200 games. Score / decisive.

| arm | greedy | JakeMan | clone | `ppo-adder3` |
|---|---|---|---|---|
| ctl s7 | 87.5/83.5 | 7.8/7.0 | 41.0/33.5 | 40.8/35.5 |
| ctl s43 | 95.0/90.5 | 9.0/8.5 | 70.0/65.0 | 63.5/57.5 |
| ctl s101 | 95.5/91.5 | 16.0/14.0 | 68.5/60.0 | 65.0/60.0 |
| ctl s202 | 97.0/95.0 | 11.0/10.5 | 59.5/54.0 | 67.5/60.5 |
| mix s7 | 96.5/85.5 | 11.0/10.0 | 61.0/44.0 | 64.5/52.5 |
| **mix s43** | 95.0/90.5 | 16.5/16.5 | 67.5/64.5 | **74.0/69.0** |
| mix s101 | 91.2/84.5 | 14.5/11.5 | 66.0/55.0 | 67.5/59.0 |
| mix s202 | 91.8/89.0 | **20.0/18.5** | 62.5/57.0 | 73.5/67.0 |

**The mixed group wins the columns that have headroom.** Group means, score
then decisive: `ppo-adder3` 69.9/61.9 against 59.2/53.4; JakeMan 15.5/14.1
against 11.0/10.0; clone 64.3/55.1 against 59.8/53.1; `greedy` 93.6/87.4
against 93.8/90.1. On `ppo-adder3` three mixed arms of four beat every control
arm. Spreads run the other way from what a harder task predicts: on
`ppo-adder3` the controls span 26.7 points (the 27-point spread again) and the
mixed arms 9.5; on the clone 29.0 against 6.5. Mixed peaks at iterations 50,
50, 70, 80; control at 80, 80, 100, 140.

**An Adder-only rung already plays every CO.** As Jake, Koal, Jess and Grimm
against `greedy`, the controls rate 87.0-97.5 and the mixed arms 90.5-96.5 --
group means 93.8 and 94.2. The modifier encoding transfers with no gradient
ever spent on the other four, which is the bet holding harder than asked. It
also means `greedy` is saturated for every CO and this instrument cannot
separate the groups; the per-CO column needs JakeMan.

**Nobody plays their CO differently.** Indirect share as Jake is no higher than
as Adder in any arm (mixed s7: 2.4% against 2.9%; human 3.9%). Pops fall from
~6.3 a game as Adder to ~4.6 as Jake/Koal/Jess and ~4.0 as Grimm in every arm
of both groups -- that is the meters, not the policy. Both groups carry PPO's
known drift equally: Anti-Air 18-24% of builds against the corpus's 6.1%,
Infantry 33-43% against 50.4%, direct engagements worth +1,880 to +2,070 an
attack against the clone's +1,557.

**One arm farms the cap.** `mix-s7` reads 96.5 on `greedy` and 61.0/64.5 on
the nets, and caps 19.0% of games against both with the group's lowest decisive
share, 44.0 on the clone. Score alone would have ranked it second.

**What it does not settle.** Why mixing helps *as Adder* against Adder
opponents is unmeasured -- opponent diversity as a regulariser is the obvious
reading and nothing here tests it. And the panel's two scripted members are at
their ends: `greedy` saturated for every CO, JakeMan 7.8-20.0 for every arm.
