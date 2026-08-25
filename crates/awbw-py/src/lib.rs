//! A batched AWBW environment for Python.
//!
//! Games are stepped in lockstep, a whole batch per call, because a Python call
//! costs microseconds and an engine step costs nanoseconds — one call per game
//! per step would spend all its time crossing the boundary.
//!
//! The action space is autoregressive, so a step takes four calls:
//!
//! ```text
//!   source_mask()                      -> sample a tile to act with
//!   dest_mask(sources)                 -> sample where it ends up
//!   kind_mask(sources, dests)          -> sample what it does
//!   param_mask(sources, dests, kinds)  -> sample the target / unit type
//!   step(sources, dests, kinds, params)
//! ```
//!
//! Each of those is batched, so the per-game cost of the round trips is small.
//! Every mask is derived from the engine's own legality checks, so a policy that
//! samples only where a mask is true can never submit an illegal order.

use awbw_engine::actions::{Action, Engine};
use awbw_engine::encoding::{
    decode, encode_observation, end_turn_source, head_sizes, observation_len, ActionCode,
    ActionMasks,
};
use awbw_engine::state::{GameState, Outcome, PlayerId};
use awbw_bots::arena::Board;
use awbw_bots::awbw_map::{AwbwMap, RIVER_SUPREME};

use numpy::ndarray::Array2;
use numpy::{IntoPyArray, PyArray1, PyArray2, PyReadonlyArray1, PyReadwriteArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// One game plus the bookkeeping the trainer needs.
struct Game {
    engine: Engine,
    /// Advantage at the last observation, for shaped rewards.
    last_advantage: f32,
    steps: u32,
}

/// Material and property, in funds, from `player`'s side of the board.
fn advantage(state: &GameState, player: PlayerId) -> f32 {
    let value = |p: PlayerId| -> f32 {
        let units: u32 = state
            .units_of(p)
            .map(|u| u.typ.stats().cost * u.hp100 as u32 / 100)
            .sum();
        units as f32 + state.property_count(p) as f32 * 5_000.0
    };
    let them = (0..state.players.len() as PlayerId)
        .find(|&p| state.are_enemies(player, p))
        .unwrap_or(player);
    (value(player) - value(them)) / 50_000.0
}

/// A batch of AWBW games.
#[pyclass]
pub struct VecEnv {
    games: Vec<Game>,
    masks: Vec<ActionMasks>,
    board: Board,
    max_day: u16,
    fog: bool,
    seed: u64,
    /// Weight on the change in material advantage. Zero trains on the win
    /// signal alone, which is correct but very sparse over a thousand-step game.
    shaping: f32,
    episodes: u64,
}

impl VecEnv {
    fn new_game(&self, index: usize, episode: u64) -> Game {
        let state = self.board.new_state(self.fog);
        let seed = self
            .seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(index as u64)
            .wrapping_add(episode.wrapping_mul(0x1000_0001));
        let engine = Engine::new(state, seed);
        let last_advantage = advantage(&engine.state, engine.state.current);
        Game {
            engine,
            last_advantage,
            steps: 0,
        }
    }

    fn obs_len(&self) -> usize {
        observation_len(&self.games[0].engine.state)
    }

    fn tiles(&self) -> usize {
        self.games[0].engine.state.map.tile_count()
    }

    fn write_obs(&self, data: &mut [f32]) {
        let len = self.obs_len();
        for (i, game) in self.games.iter().enumerate() {
            encode_observation(
                &game.engine.state,
                game.engine.vision(),
                &mut data[i * len..(i + 1) * len],
            );
        }
    }

    /// The game is over, either decided or out of time.
    fn is_finished(state: &GameState, max_day: u16) -> bool {
        state.outcome() != Outcome::InProgress || state.day > max_day
    }
}

fn check_len(name: &str, got: usize, want: usize) -> PyResult<()> {
    if got != want {
        return Err(PyValueError::new_err(format!(
            "{name} has {got} entries, expected {want}"
        )));
    }
    Ok(())
}

#[pymethods]
impl VecEnv {
    #[new]
    /// `map_path` defaults to the committed league map; pass `None` for the
    /// synthetic board, which needs no data file.
    #[pyo3(signature = (num_envs, seed=0, max_day=60, fog=false, shaping=0.0, map_path=None))]
    fn new(
        num_envs: usize,
        seed: u64,
        max_day: u16,
        fog: bool,
        shaping: f32,
        map_path: Option<String>,
    ) -> PyResult<Self> {
        if num_envs == 0 {
            return Err(PyValueError::new_err("num_envs must be positive"));
        }
        let board = match map_path.as_deref() {
            Some("synthetic") => Board::default(),
            other => {
                let path = other.unwrap_or(RIVER_SUPREME);
                Board::Awbw(Box::new(AwbwMap::load(path).map_err(PyValueError::new_err)?))
            }
        };

        let mut env = VecEnv {
            games: Vec::new(),
            masks: Vec::new(),
            board,
            max_day,
            fog,
            seed,
            shaping,
            episodes: 0,
        };
        for i in 0..num_envs {
            let state = env.board.new_state(fog);
            let engine = Engine::new(state, seed.wrapping_add(i as u64));
            let last_advantage = advantage(&engine.state, engine.state.current);
            env.games.push(Game { engine, last_advantage, steps: 0 });
            env.masks.push(ActionMasks::new());
        }
        Ok(env)
    }

    /// The board these games are played on.
    #[getter]
    fn map_name(&self) -> String {
        self.board.name().to_string()
    }

    #[getter]
    fn num_envs(&self) -> usize {
        self.games.len()
    }

    /// Floats per observation.
    #[getter]
    fn observation_size(&self) -> usize {
        self.obs_len()
    }

    /// Logit counts for the four heads: source, destination, kind, parameter.
    #[getter]
    fn action_sizes(&self) -> [usize; 4] {
        head_sizes(&self.games[0].engine.state)
    }

    #[getter]
    fn board_shape(&self) -> (u8, u8) {
        let map = &self.games[0].engine.state.map;
        (map.height, map.width)
    }

    /// The index that means "end the turn".
    #[getter]
    fn end_turn_index(&self) -> u32 {
        end_turn_source(&self.games[0].engine.state)
    }

    /// Which seat is to move in each game.
    fn current_player<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<i64>> {
        let data: Vec<i64> = self
            .games
            .iter()
            .map(|g| g.engine.state.current as i64)
            .collect();
        data.into_pyarray(py)
    }

    /// Starts every game over and returns the first observations.
    fn reset<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        self.episodes += 1;
        for i in 0..self.games.len() {
            self.games[i] = self.new_game(i, self.episodes);
        }
        self.observe(py)
    }

    /// Writes the current observations into a caller-owned array.
    ///
    /// The allocating `observe` returns several megabytes per batch step, which
    /// is the single largest cost of crossing into Python; a trainer that keeps
    /// one buffer and refills it avoids all of it.
    fn observe_into(&self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let expected = self.games.len() * self.obs_len();
        let data = out
            .as_slice_mut()
            .map_err(|_| PyValueError::new_err("buffer must be C-contiguous"))?;
        if data.len() != expected {
            return Err(PyValueError::new_err(format!(
                "buffer holds {} floats, expected {expected}",
                data.len()
            )));
        }
        self.write_obs(data);
        Ok(())
    }

    /// Current observations, `(num_envs, observation_size)`.
    fn observe<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        let len = self.obs_len();
        let mut data = vec![0.0f32; self.games.len() * len];
        self.write_obs(&mut data);
        Array2::from_shape_vec((self.games.len(), len), data)
            .expect("observation buffer is rectangular")
            .into_pyarray(py)
    }

    /// Which tiles can act, plus the end-turn index.
    fn source_mask<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<bool>> {
        let width = self.tiles() + 1;
        let mut data = vec![false; self.games.len() * width];
        let mut scratch = Vec::new();
        for (i, game) in self.games.iter_mut().enumerate() {
            self.masks[i].source_mask(&mut game.engine, &mut scratch);
            data[i * width..i * width + scratch.len()].copy_from_slice(&scratch);
        }
        Array2::from_shape_vec((self.games.len(), width), data)
            .expect("mask buffer is rectangular")
            .into_pyarray(py)
    }

    /// Where the chosen tile's unit may end up.
    fn dest_mask<'py>(
        &mut self,
        py: Python<'py>,
        sources: PyReadonlyArray1<u32>,
    ) -> PyResult<Bound<'py, PyArray2<bool>>> {
        let sources = sources.as_slice()?;
        check_len("sources", sources.len(), self.games.len())?;
        let width = self.tiles();
        let mut data = vec![false; self.games.len() * width];
        let mut scratch = Vec::new();
        for (i, game) in self.games.iter_mut().enumerate() {
            self.masks[i].dest_mask(&mut game.engine, sources[i], &mut scratch);
            data[i * width..i * width + scratch.len()].copy_from_slice(&scratch);
        }
        Ok(Array2::from_shape_vec((self.games.len(), width), data)
            .expect("mask buffer is rectangular")
            .into_pyarray(py))
    }

    /// What that unit may do at that destination.
    fn kind_mask<'py>(
        &mut self,
        py: Python<'py>,
        sources: PyReadonlyArray1<u32>,
        dests: PyReadonlyArray1<u32>,
    ) -> PyResult<Bound<'py, PyArray2<bool>>> {
        let (sources, dests) = (sources.as_slice()?, dests.as_slice()?);
        check_len("sources", sources.len(), self.games.len())?;
        check_len("dests", dests.len(), self.games.len())?;
        let width = self.action_sizes()[2];
        let mut data = vec![false; self.games.len() * width];
        let mut scratch = Vec::new();
        for (i, game) in self.games.iter_mut().enumerate() {
            self.masks[i].kind_mask(&mut game.engine, sources[i], dests[i], &mut scratch);
            data[i * width..i * width + scratch.len()].copy_from_slice(&scratch);
        }
        Ok(Array2::from_shape_vec((self.games.len(), width), data)
            .expect("mask buffer is rectangular")
            .into_pyarray(py))
    }

    /// The attack target, unit type to build, or passenger and direction.
    fn param_mask<'py>(
        &mut self,
        py: Python<'py>,
        sources: PyReadonlyArray1<u32>,
        dests: PyReadonlyArray1<u32>,
        kinds: PyReadonlyArray1<u32>,
    ) -> PyResult<Bound<'py, PyArray2<bool>>> {
        let (sources, dests, kinds) = (sources.as_slice()?, dests.as_slice()?, kinds.as_slice()?);
        check_len("sources", sources.len(), self.games.len())?;
        check_len("dests", dests.len(), self.games.len())?;
        check_len("kinds", kinds.len(), self.games.len())?;
        let width = self.action_sizes()[3];
        let mut data = vec![false; self.games.len() * width];
        let mut scratch = Vec::new();
        for (i, game) in self.games.iter_mut().enumerate() {
            self.masks[i].param_mask(
                &mut game.engine,
                sources[i],
                dests[i],
                kinds[i] as u8,
                &mut scratch,
            );
            data[i * width..i * width + scratch.len()].copy_from_slice(&scratch);
        }
        Ok(Array2::from_shape_vec((self.games.len(), width), data)
            .expect("mask buffer is rectangular")
            .into_pyarray(py))
    }

    /// Applies one order per game.
    ///
    /// Returns `(rewards, dones, acting_player)`. Rewards are from the side
    /// that just moved. A finished game restarts immediately, so the next
    /// observation for it belongs to a new episode; `dones` marks that
    /// boundary. Fetch observations with `observe` or `observe_into`.
    #[allow(clippy::type_complexity)]
    fn step<'py>(
        &mut self,
        py: Python<'py>,
        sources: PyReadonlyArray1<u32>,
        dests: PyReadonlyArray1<u32>,
        kinds: PyReadonlyArray1<u32>,
        params: PyReadonlyArray1<u32>,
    ) -> PyResult<(
        Bound<'py, PyArray1<f32>>,
        Bound<'py, PyArray1<bool>>,
        Bound<'py, PyArray1<i64>>,
    )> {
        let (sources, dests) = (sources.as_slice()?, dests.as_slice()?);
        let (kinds, params) = (kinds.as_slice()?, params.as_slice()?);
        for (name, len) in [
            ("sources", sources.len()),
            ("dests", dests.len()),
            ("kinds", kinds.len()),
            ("params", params.len()),
        ] {
            check_len(name, len, self.games.len())?;
        }

        let max_day = self.max_day;
        let shaping = self.shaping;
        let mut rewards = vec![0.0f32; self.games.len()];
        let mut dones = vec![false; self.games.len()];
        let mut actors = vec![0i64; self.games.len()];
        let mut restarts = Vec::new();

        for i in 0..self.games.len() {
            let game = &mut self.games[i];
            let actor = game.engine.state.current;
            actors[i] = actor as i64;

            let code = ActionCode {
                source: sources[i],
                dest: dests[i],
                kind: kinds[i] as u8,
                param: params[i],
            };
            // An unmasked or stale selection forfeits the turn rather than
            // failing the whole batch; a policy sampling under the masks will
            // never land here.
            let action = decode(&game.engine.state, code).unwrap_or(Action::EndTurn);
            if game.engine.apply(action).is_err() {
                let _ = game.engine.apply(Action::EndTurn);
            }
            game.steps += 1;

            let advantage_now = advantage(&game.engine.state, actor);
            rewards[i] = shaping * (advantage_now - game.last_advantage);
            game.last_advantage = advantage_now;

            if VecEnv::is_finished(&game.engine.state, max_day) {
                dones[i] = true;
                rewards[i] += match game.engine.state.outcome() {
                    Outcome::Winner(p) if p == actor => 1.0,
                    Outcome::Winner(_) => -1.0,
                    _ => 0.0,
                };
                restarts.push(i);
            }
        }

        self.episodes += 1;
        for i in restarts {
            self.games[i] = self.new_game(i, self.episodes);
        }
        // Every game's advantage baseline follows whoever is now to move.
        for game in self.games.iter_mut() {
            game.last_advantage = advantage(&game.engine.state, game.engine.state.current);
        }

        Ok((
            rewards.into_pyarray(py),
            dones.into_pyarray(py),
            actors.into_pyarray(py),
        ))
    }

    fn __repr__(&self) -> String {
        let (h, w) = self.board_shape();
        format!(
            "VecEnv(num_envs={}, map={:?} {h}x{w}, fog={}, max_day={}, shaping={})",
            self.games.len(),
            self.board.name(),
            self.fog,
            self.max_day,
            self.shaping
        )
    }
}

#[pymodule]
fn awbw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VecEnv>()?;
    m.add("__doc__", "Batched Advance Wars by Web environment.")?;
    Ok(())
}
