//! A symmetric map for matches.
//!
//! Bots are compared on a board that is fair by construction: rotate it 180
//! degrees and it maps onto itself, so neither seat has terrain to thank for a
//! result. Real AWBW maps are the better test eventually, but they are not in
//! the repository and a match harness should not need a download to run.
//!
//! Each side starts with an HQ, two bases and two cities, with a contested band
//! of neutral cities across the middle. There are deliberately **no airports or
//! ports**, so matches are land-only: it keeps the action space small while the
//! ladder is being calibrated. `arena --show-map` prints the board.

use std::sync::Arc;

use awbw_engine::map::Map;
use awbw_engine::state::PlayerId;
use awbw_engine::types::TerrainKind;

/// Builds a rotationally symmetric two-player map, and the property owners that
/// go with it.
pub fn symmetric_map(width: u8, height: u8) -> (Arc<Map>, Vec<Option<PlayerId>>) {
    let w = width as usize;
    let h = height as usize;
    let mut kinds = vec![TerrainKind::Plain; w * h];

    // Terrain first, mirrored through the centre so both halves match.
    for y in 0..h / 2 + 1 {
        for x in 0..w {
            let kind = if (x * 5 + y * 3) % 11 == 0 {
                TerrainKind::Mountain
            } else if (x * 3 + y * 7) % 9 < 2 {
                TerrainKind::Wood
            } else if y == h / 2 && x % 4 == 1 {
                TerrainKind::Road
            } else {
                TerrainKind::Plain
            };
            kinds[y * w + x] = kind;
            kinds[(h - 1 - y) * w + (w - 1 - x)] = kind;
        }
    }

    // Then the properties, in mirrored pairs.
    let place = |kinds: &mut Vec<TerrainKind>, x: usize, y: usize, kind: TerrainKind| {
        kinds[y * w + x] = kind;
        kinds[(h - 1 - y) * w + (w - 1 - x)] = kind;
    };
    place(&mut kinds, 1, 1, TerrainKind::Hq);
    place(&mut kinds, 2, 1, TerrainKind::Base);
    place(&mut kinds, 1, 2, TerrainKind::Base);
    place(&mut kinds, 3, 1, TerrainKind::City);
    place(&mut kinds, 1, 3, TerrainKind::City);
    for i in 0..3 {
        place(&mut kinds, 2 + i * 3, h / 2, TerrainKind::City);
        place(&mut kinds, 4, 4 + i, TerrainKind::City);
    }

    let map = Map::from_kinds(width, height, kinds).expect("symmetric map is well formed");

    // A property belongs to whichever half it sits in; the contested middle row
    // starts neutral.
    let owners = map
        .properties()
        .iter()
        .map(|p| {
            let y = p.pos.y as usize;
            let near_own_hq = y < h / 2 && (p.pos.x as usize) < w / 2;
            let near_other_hq = y > h / 2 && (p.pos.x as usize) > w / 2;
            if near_own_hq && y <= 3 {
                Some(0)
            } else if near_other_hq && y >= h - 4 {
                Some(1)
            } else {
                None
            }
        })
        .collect();

    (Arc::new(map), owners)
}

/// Renders a map as ASCII, so a match result can be read against the board it
/// was played on rather than taken on trust.
///
/// Every symbol is lower case by default and upper-cased for seat 0, so
/// ownership is legible on properties; neutral property stays lower case, like
/// the ground.
pub fn render(map: &Map, owners: &[Option<PlayerId>]) -> String {
    use awbw_engine::map::Pos;
    use std::collections::HashMap;

    let owner_at: HashMap<Pos, Option<PlayerId>> = map
        .properties()
        .iter()
        .zip(owners.iter())
        .map(|(seed, owner)| (seed.pos, *owner))
        .collect();

    let mut out = String::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Pos::new(x, y);
            let symbol = match map.terrain_at(pos) {
                TerrainKind::Plain => '.',
                TerrainKind::Wood => 'w',
                TerrainKind::Mountain => '^',
                TerrainKind::Road => '-',
                TerrainKind::Bridge => '=',
                TerrainKind::River => 'r',
                TerrainKind::Sea => '~',
                TerrainKind::Shoal => 's',
                TerrainKind::Reef => 'o',
                TerrainKind::Hq => 'q',
                TerrainKind::Base => 'b',
                TerrainKind::City => 'c',
                TerrainKind::Airport => 'a',
                TerrainKind::Port => 'p',
                TerrainKind::ComTower => 't',
                TerrainKind::Lab => 'l',
                TerrainKind::Silo | TerrainKind::SiloEmpty => 'i',
                TerrainKind::Pipe => '#',
                TerrainKind::PipeSeam | TerrainKind::PipeRubble => '+',
                TerrainKind::Teleporter => '@',
            };
            out.push(match owner_at.get(&pos).copied().flatten() {
                Some(0) => symbol.to_ascii_uppercase(),
                _ => symbol,
            });
        }
        out.push('\n');
    }
    out
}

/// The legend for [`render`].
pub const RENDER_LEGEND: &str = "  . plain   w wood    ^ mountain  - road    = bridge  r river
  ~ sea     s shoal   o reef      # pipe    + seam    @ teleporter
  q hq      b base    c city      a airport p port    t tower   l lab   i silo
  UPPER CASE = seat 0, lower = seat 1 or neutral";

#[cfg(test)]
mod tests {
    use super::*;
    use awbw_engine::map::Pos;

    #[test]
    fn the_board_is_symmetric_under_rotation() {
        let (map, _) = symmetric_map(13, 13);
        for y in 0..13u8 {
            for x in 0..13u8 {
                let here = map.terrain_at(Pos::new(x, y));
                let there = map.terrain_at(Pos::new(12 - x, 12 - y));
                assert_eq!(here, there, "asymmetry at ({x},{y})");
            }
        }
    }

    #[test]
    fn each_side_starts_with_an_hq_and_production() {
        let (map, owners) = symmetric_map(13, 13);
        let mut mine = 0;
        let mut theirs = 0;
        for (seed, owner) in map.properties().iter().zip(owners.iter()) {
            let _ = seed;
            match owner {
                Some(0) => mine += 1,
                Some(1) => theirs += 1,
                None => {}
                _ => panic!("unexpected owner"),
            }
        }
        assert_eq!(mine, theirs, "starting property counts must match");
        assert!(mine >= 3, "each side needs an HQ and something to build from");
    }
}
