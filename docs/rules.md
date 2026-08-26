# Rules

> What the engine models, what it deliberately does not, and where the numbers
> came from. The wiki is authoritative; see `decisions.md`.

## Sources

| data | source |
|---|---|
| Damage matrix | `awbw.amarriner.com/js/damage_inc.json` — the site's own file, verbatim |
| Unit stats | `units.php` chart page |
| Movement costs, terrain defence | `terrain.php` chart page (Clear/Rain/Snow) |
| Terrain id → kind (196 ids) | AWBW DB dump, via RizeBot's generated table |
| Damage formula | AWBW's server engine (`helper/fire.rs`), documented in RizeBot's `damage.ts` |
| CO abilities | AWBW-Replay-Player's `COs.json`, plus `co.php` for what it omits |

Raw copies live in `data/awbw-site/`.

## Combat

AWBW's formula, which differs from the cartridge in ways that matter:

- Luck is +0..9% **inclusive**.
- Damage is computed to one decimal, then truncated — only `x.95` and up gains
  a point.
- Displayed HP is `ceil(hp/10)` and feeds **both** attack scaling and the
  defender's terrain cover, so a wounded defender gets less cover.
- Air units and pipe seams get zero terrain stars.
- The primary weapon is used when it has an entry and the attacker has ammo;
  otherwise the secondary.
- Unloading is a **free action**: a transport may unload after moving, and
  unloading does not end its turn. The cartridge bundles the two; AWBW does not.
- Com Towers add +10% attack each.

## Implemented

Movement with per-terrain, per-weather costs and fuel as a second budget;
combat with counterattacks; capture; production; transports (load, unload,
capacity and cargo rules); join with funds refund; APC supply; turn bookkeeping
(income, repair and resupply, fuel upkeep, air and sea crashes); win by HQ
capture, annihilation or capture limit; fog of war with cover, concealment and
ambush.

**CO day-to-day abilities**: per-unit attack, defence and range deltas, build
cost multipliers, terrain-conditional bonuses (Kindle on properties, Koal on
roads, Jake on plains, Lash per terrain star), Sami's capture rate and
transport movement, Rachel's extra repair, Sasha's property income, Eagle's air
fuel saving, and luck ranges.

## Not implemented

Deliberate. Divergences involving these are expected, not bugs.

- **CO powers.** The largest known gap. The agent will never use one and their
  absence is symmetric in self-play.
- **Missile silos, pipe-seam destruction, teleporters.** Absent from the
  competitive map pool sampled so far.
- **Mid-game weather**, including powers that cause it (Drake's Typhoon), and
  Olaf's weather remap.
- **Lab-gated units.** Labs are capturable and repair, but do not unlock units.
- **Sonja's fog effects** and **Javier's defence against indirects** — the
  latter is conditional on the *attacker* being indirect, which the per-unit
  CO table cannot express.
- **Black Bomb detonation.**

## Unverified

Nell, Flak and Jugger's luck ranges are implemented from the CO table but no
recorded game exercises them: they are banned in Global League play.
