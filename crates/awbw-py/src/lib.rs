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
    decode, encode, encode_observation, end_turn_source, head_sizes, observation_len, ActionCode,
    ActionMasks,
};
use awbw_engine::rng::Rng;
use awbw_engine::state::{GameState, Outcome, PlayerId};
use awbw_bots::arena::Board;
use awbw_bots::awbw_map::{AwbwMap, RIVER_SUPREME};
use awbw_bots::greedy::GreedyBot;
use awbw_bots::{Bot, RandomBot};
use awbw_replay::imitate::Cursor;
use awbw_replay::schema::Replay;
use awbw_replay::Verifier;

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

/// A batch of games played by a scripted teacher, for behaviour cloning.
///
/// No dataset files. An observation is 19k floats, so a million samples would
/// be seventy-odd gigabytes on disk, while the engine regenerates them at tens
/// of thousands a second -- storing them would cost more than making them. The
/// teacher plays continuously and hands back what it did, so the data is
/// unlimited and never repeats.
///
/// ```text
///   env.observe_into(obs)   # the positions the teacher is about to act on
///   targets = env.act()     # what it chose, applied; (num_envs, 4) codes
/// ```
#[pyclass]
pub struct TeacherEnv {
    inner: VecEnv,
    teachers: Vec<Box<dyn Bot + Send + Sync>>,
    /// Games finished since construction, and how many the first seat won.
    finished: u64,
    seat_zero_wins: u64,
}

fn make_teacher(name: &str, seed: u64) -> PyResult<Box<dyn Bot + Send + Sync>> {
    Ok(match name {
        "greedy" => Box::new(GreedyBot::new()),
        "capturer" => Box::new(GreedyBot::capture_only()),
        "random" => Box::new(RandomBot::new(seed)),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown teacher {other:?}; expected greedy, capturer or random"
            )))
        }
    })
}

#[pymethods]
impl TeacherEnv {
    #[new]
    #[pyo3(signature = (num_envs, teacher="greedy", seed=0, max_day=60, fog=false, map_path=None))]
    fn new(
        num_envs: usize,
        teacher: &str,
        seed: u64,
        max_day: u16,
        fog: bool,
        map_path: Option<String>,
    ) -> PyResult<Self> {
        let inner = VecEnv::new(num_envs, seed, max_day, fog, 0.0, map_path)?;
        let teachers = (0..num_envs)
            .map(|i| make_teacher(teacher, seed.wrapping_add(i as u64)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(TeacherEnv {
            inner,
            teachers,
            finished: 0,
            seat_zero_wins: 0,
        })
    }

    #[getter]
    fn num_envs(&self) -> usize {
        self.inner.games.len()
    }

    #[getter]
    fn observation_size(&self) -> usize {
        self.inner.obs_len()
    }

    #[getter]
    fn action_sizes(&self) -> [usize; 4] {
        self.inner.action_sizes()
    }

    #[getter]
    fn board_shape(&self) -> (u8, u8) {
        self.inner.board_shape()
    }

    #[getter]
    fn map_name(&self) -> String {
        self.inner.map_name()
    }

    /// Games completed so far, and the fraction the first seat won. A teacher
    /// mirror-matched against itself should sit near half, and a number far
    /// from it means the map or the seating is lopsided.
    #[getter]
    fn stats(&self) -> (u64, f64) {
        let rate = if self.finished == 0 {
            0.5
        } else {
            self.seat_zero_wins as f64 / self.finished as f64
        };
        (self.finished, rate)
    }

    fn observe_into(&self, out: PyReadwriteArray2<f32>) -> PyResult<()> {
        self.inner.observe_into(out)
    }

    fn observe<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        self.inner.observe(py)
    }

    /// Which seat is to move in each game, so a trainer can tell the two sides
    /// of a self-play game apart.
    fn current_player<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<i64>> {
        self.inner.current_player(py)
    }

    /// Lets the teacher move once in every game, and returns what it chose as
    /// `(num_envs, 4)` action codes — the labels to clone.
    fn act<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<u32>> {
        let mut codes = vec![0u32; self.inner.games.len() * 4];
        let mut restarts = Vec::new();

        for i in 0..self.inner.games.len() {
            let game = &mut self.inner.games[i];
            let actor = game.engine.state.current;
            let action = self.teachers[i].choose(&mut game.engine);

            // Record before applying: the label belongs to the position the
            // observation was taken from.
            let code = encode(&game.engine.state, action).unwrap_or(ActionCode {
                source: end_turn_source(&game.engine.state),
                dest: 0,
                kind: 0,
                param: 0,
            });
            codes[i * 4] = code.source;
            codes[i * 4 + 1] = code.dest;
            codes[i * 4 + 2] = code.kind as u32;
            codes[i * 4 + 3] = code.param;

            if game.engine.apply(action).is_err() {
                let _ = game.engine.apply(Action::EndTurn);
            }

            if VecEnv::is_finished(&game.engine.state, self.inner.max_day) {
                self.finished += 1;
                if let Outcome::Winner(0) = game.engine.state.outcome() {
                    self.seat_zero_wins += 1;
                }
                let _ = actor;
                restarts.push(i);
            }
        }

        self.inner.episodes += 1;
        for i in restarts {
            self.inner.games[i] = self.inner.new_game(i, self.inner.episodes);
            self.teachers[i].reset(self.inner.seed.wrapping_add(self.inner.episodes));
        }

        Array2::from_shape_vec((self.inner.games.len(), 4), codes)
            .expect("code buffer is rectangular")
            .into_pyarray(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "TeacherEnv(num_envs={}, map={:?}, games_finished={})",
            self.inner.games.len(),
            self.inner.board.name(),
            self.finished
        )
    }
}

/// A batch of *recorded human games*, served with the same interface as
/// `TeacherEnv`, for behaviour cloning off the replay corpus.
///
/// The scripted teacher plays a decent game; humans play a much better one, and
/// there are only so many of their games. So this reads them in the trainer's
/// own currency — a position and the order a person gave in it — while
/// `TeacherEnv` supplies unlimited weaker data from the same interface.
///
/// Two filters matter, both on by default. Orders played while a CO power was
/// running are dropped: the engine does not model powers, so the position does
/// not explain the choice. Orders the engine rejects are dropped too, since an
/// order the engine cannot even reproduce is not something a masked policy
/// could ever emit.
///
/// Every game must be on the same map — observations are board-shaped, so a
/// batch cannot mix sizes.
///
/// ```text
///   env.observe_into(obs)        # positions humans were about to act on
///   codes, valid = env.act()     # what they chose; (num_envs, 4) and (num_envs,)
/// ```
#[pyclass]
pub struct ReplayTeacher {
    files: Vec<std::path::PathBuf>,
    next_file: usize,
    slots: Vec<Option<Cursor>>,
    want_map: Option<String>,
    shape: (u8, u8),
    obs_len: usize,
    sizes: [usize; 4],
    end_turn: u32,
    map_name: String,
    skip_powers: bool,
    skip_illegal: bool,
    served: u64,
    skipped_power: u64,
    skipped_illegal: u64,
    games_opened: u64,
    epochs: u64,
}

impl ReplayTeacher {
    /// Parses replays in shuffled order until one matches the batch's map, and
    /// hands back a cursor over it. `None` once a whole sweep finds nothing.
    fn open_next(&mut self) -> Option<Cursor> {
        for _ in 0..self.files.len() {
            let index = self.next_file;
            self.next_file += 1;
            if self.next_file >= self.files.len() {
                self.next_file = 0;
                self.epochs += 1;
            }

            let Ok(text) = std::fs::read_to_string(&self.files[index]) else {
                continue;
            };
            // Parsing a replay costs about twenty times reading it, and on a
            // mixed corpus most files are on the wrong map. Looking for the
            // name in the raw text first skips those for almost nothing; a
            // false positive is caught by the real check below.
            if let Some(want) = &self.want_map {
                if !text.contains(want.as_str()) {
                    continue;
                }
            }
            let Ok(replay) = serde_json::from_str::<Replay>(&text) else {
                continue;
            };
            if let Some(want) = &self.want_map {
                if replay.map_name.as_deref() != Some(want.as_str()) {
                    continue;
                }
            }
            // The first game fixes the board; anything else must match it.
            let (h, w) = (replay.height as u8, replay.width as u8);
            if self.shape != (0, 0) && (h, w) != self.shape {
                continue;
            }
            let name = replay.map_name.clone().unwrap_or_default();
            let Ok(verifier) = Verifier::new(std::sync::Arc::new(replay)) else {
                continue;
            };
            self.games_opened += 1;
            if self.shape == (0, 0) {
                self.shape = (h, w);
                self.map_name = name;
            }
            return Some(Cursor::new(verifier));
        }
        None
    }

    /// Walks a slot forward until it sits on an order worth learning from,
    /// opening fresh games as each one runs out.
    fn refill(&mut self, slot: usize) {
        let mut cursor = self.slots[slot].take();
        let mut opens = 0usize;
        let limit = self.files.len().max(1);

        loop {
            if cursor.is_none() {
                if opens >= limit {
                    break;
                }
                opens += 1;
                cursor = self.open_next();
                if cursor.is_none() {
                    break;
                }
            }

            let found = {
                let c = cursor.as_mut().expect("cursor present");
                if c.finished() {
                    None
                } else {
                    c.sample()
                }
            };
            let Some(sample) = found else {
                cursor = None;
                continue;
            };

            let skip = (self.skip_powers && sample.power_active)
                || (self.skip_illegal && !(sample.legal && sample.emittable));
            if !skip {
                break;
            }
            if sample.power_active {
                self.skipped_power += 1;
            } else {
                self.skipped_illegal += 1;
            }
            cursor.as_mut().expect("cursor present").advance();
        }

        self.slots[slot] = cursor;
    }
}

#[pymethods]
impl ReplayTeacher {
    #[new]
    /// `map_name` of `None` takes whichever map the first replay uses and holds
    /// the rest of the batch to it.
    #[pyo3(signature = (
        replay_dir="data/prepared",
        num_envs=32,
        map_name="A River Supreme",
        seed=0,
        skip_powers=true,
        skip_illegal=true,
    ))]
    fn new(
        replay_dir: &str,
        num_envs: usize,
        map_name: Option<&str>,
        seed: u64,
        skip_powers: bool,
        skip_illegal: bool,
    ) -> PyResult<Self> {
        if num_envs == 0 {
            return Err(PyValueError::new_err("num_envs must be positive"));
        }
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(replay_dir)
            .map_err(|e| PyValueError::new_err(format!("{replay_dir}: {e}")))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        if files.is_empty() {
            return Err(PyValueError::new_err(format!("no replays in {replay_dir}")));
        }
        files.sort();
        // Shuffled, so slots start in different games and a batch is not
        // thirty-two views of one opening.
        let mut rng = Rng::new(seed);
        for i in (1..files.len()).rev() {
            files.swap(i, rng.roll_inclusive(i as u32) as usize);
        }

        let mut teacher = ReplayTeacher {
            files,
            next_file: 0,
            slots: (0..num_envs).map(|_| None).collect(),
            want_map: map_name.map(str::to_string),
            shape: (0, 0),
            obs_len: 0,
            sizes: [0; 4],
            end_turn: 0,
            map_name: String::new(),
            skip_powers,
            skip_illegal,
            served: 0,
            skipped_power: 0,
            skipped_illegal: 0,
            games_opened: 0,
            epochs: 0,
        };

        for i in 0..num_envs {
            teacher.refill(i);
        }
        let Some(state) = teacher.slots.iter().find_map(|s| s.as_ref()?.state()) else {
            return Err(PyValueError::new_err(format!(
                "no usable replays in {replay_dir}{}",
                match map_name {
                    Some(m) => format!(" on map {m:?}"),
                    None => String::new(),
                }
            )));
        };
        teacher.obs_len = observation_len(state);
        teacher.sizes = head_sizes(state);
        teacher.end_turn = end_turn_source(state);
        Ok(teacher)
    }

    #[getter]
    fn num_envs(&self) -> usize {
        self.slots.len()
    }

    #[getter]
    fn observation_size(&self) -> usize {
        self.obs_len
    }

    #[getter]
    fn action_sizes(&self) -> [usize; 4] {
        self.sizes
    }

    #[getter]
    fn board_shape(&self) -> (u8, u8) {
        self.shape
    }

    #[getter]
    fn map_name(&self) -> String {
        self.map_name.clone()
    }

    #[getter]
    fn end_turn_index(&self) -> u32 {
        self.end_turn
    }

    /// Replays available on this map, before filtering by content.
    #[getter]
    fn replay_count(&self) -> usize {
        self.files.len()
    }

    /// `(orders served, orders skipped for a power, orders skipped as illegal,
    /// games opened, passes over the corpus)`.
    ///
    /// The two skip counts are the honest cost of the filters: a rising illegal
    /// count means the engine and the record disagree about something.
    #[getter]
    fn stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.served,
            self.skipped_power,
            self.skipped_illegal,
            self.games_opened,
            self.epochs,
        )
    }

    /// Which seat is to move in each slot. `-1` where a slot has no game.
    fn current_player<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<i64>> {
        let data: Vec<i64> = self
            .slots
            .iter()
            .map(|s| {
                s.as_ref()
                    .and_then(|c| c.current_player())
                    .map_or(-1, |p| p as i64)
            })
            .collect();
        data.into_pyarray(py)
    }

    /// Writes the positions the recorded orders were played in. Rows for
    /// exhausted slots are zeroed, and `act` marks them invalid.
    fn observe_into(&mut self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let expected = self.slots.len() * self.obs_len;
        let len = self.obs_len;
        let data = out
            .as_slice_mut()
            .map_err(|_| PyValueError::new_err("buffer must be C-contiguous"))?;
        if data.len() != expected {
            return Err(PyValueError::new_err(format!(
                "buffer holds {} floats, expected {expected}",
                data.len()
            )));
        }
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let row = &mut data[i * len..(i + 1) * len];
            let written = match slot {
                Some(cursor) => cursor.observe(row),
                None => false,
            };
            if !written {
                row.fill(0.0);
            }
        }
        Ok(())
    }

    /// Current observations, `(num_envs, observation_size)`.
    fn observe<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        let len = self.obs_len;
        let mut data = vec![0.0f32; self.slots.len() * len];
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(cursor) = slot {
                cursor.observe(&mut data[i * len..(i + 1) * len]);
            }
        }
        Array2::from_shape_vec((self.slots.len(), len), data)
            .expect("observation buffer is rectangular")
            .into_pyarray(py)
    }

    /// The orders the humans gave, as `(num_envs, 4)` action codes, plus a
    /// `(num_envs,)` mask of which rows carry one. Advances every slot.
    fn act<'py>(&mut self, py: Python<'py>) -> (Bound<'py, PyArray2<u32>>, Bound<'py, PyArray1<bool>>) {
        let n = self.slots.len();
        let mut codes = vec![0u32; n * 4];
        let mut valid = vec![false; n];

        for i in 0..n {
            let sample = self.slots[i].as_mut().and_then(|c| c.sample());
            if let Some(sample) = sample {
                codes[i * 4] = sample.code.source;
                codes[i * 4 + 1] = sample.code.dest;
                codes[i * 4 + 2] = sample.code.kind as u32;
                codes[i * 4 + 3] = sample.code.param;
                valid[i] = true;
                self.served += 1;
                self.slots[i].as_mut().expect("slot present").advance();
            }
            self.refill(i);
        }

        (
            Array2::from_shape_vec((n, 4), codes)
                .expect("code buffer is rectangular")
                .into_pyarray(py),
            valid.into_pyarray(py),
        )
    }

    fn __repr__(&self) -> String {
        let (h, w) = self.shape;
        format!(
            "ReplayTeacher(num_envs={}, map={:?} {h}x{w}, replays={}, served={})",
            self.slots.len(),
            self.map_name,
            self.files.len(),
            self.served
        )
    }
}

#[pymodule]
fn awbw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VecEnv>()?;
    m.add_class::<TeacherEnv>()?;
    m.add_class::<ReplayTeacher>()?;
    m.add("__doc__", "Batched Advance Wars by Web environment.")?;
    Ok(())
}
