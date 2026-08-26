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
use awbw_bots::jakeman::JakeManBot;
use awbw_bots::{Bot, RandomBot};
use awbw_replay::imitate::Cursor;
use awbw_replay::schema::Replay;
use awbw_replay::Verifier;

mod record;
use record::Recorder;

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
    /// A scripted opponent, one per game, playing the seat the agent does not.
    ///
    /// Empty for self-play, where whatever the caller submits moves both sides.
    /// With one set, the caller only ever sees its own seat's positions, which
    /// is what rating a policy against a fixed baseline needs.
    opponents: Vec<Box<dyn Bot + Send + Sync>>,
    /// Which seat the caller plays. Alternated across the batch, so a policy is
    /// rated on both sides of an asymmetric map rather than on the better one.
    agent_seats: Vec<PlayerId>,
    finished: u64,
    agent_wins: u64,
    draws: u64,
    /// Submitted orders the engine would not take, which forfeit the turn.
    ///
    /// A policy sampling under the masks should never produce one, so any
    /// number here at all means the masks and the rules disagree — and the
    /// symptom is a policy that looks merely bad rather than broken.
    rejected: u64,
    submitted: u64,
    /// One per game, kept only when the caller asks for replays. Recording
    /// costs a snapshot of every unit and building once per turn, which is
    /// nothing beside a rollout but pointless when nobody will read it.
    recorders: Vec<Recorder>,
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

    /// Lets the scripted opponent play until the agent is to move again.
    ///
    /// A whole opposing turn is many orders, so this is a loop rather than a
    /// step. The guard is for a bot that never ends its turn; the ones here
    /// always do, but a hung environment is a miserable thing to debug.
    fn run_opponent(&mut self, index: usize) {
        if self.opponents.is_empty() {
            return;
        }
        let seat = self.agent_seats[index];
        let max_day = self.max_day;
        for _ in 0..4096 {
            let game = &mut self.games[index];
            if game.engine.state.current == seat
                || VecEnv::is_finished(&game.engine.state, max_day)
            {
                return;
            }
            let action = self.opponents[index].choose(&mut game.engine);
            let recorded = !self.recorders.is_empty();
            let before = recorded
                .then(|| self.recorders[index].begin(&self.games[index].engine.state, action));
            let game = &mut self.games[index];
            let refused = game.engine.apply(action).is_err();
            if refused {
                let _ = game.engine.apply(Action::EndTurn);
            }
            if let Some(before) = before {
                let played = if refused { Action::EndTurn } else { action };
                self.recorders[index].end(&self.games[index].engine, played, before);
            }
        }
        let _ = self.games[index].engine.apply(Action::EndTurn);
    }

    /// Records a finished game from the agent's side.
    fn score(&mut self, index: usize) {
        self.finished += 1;
        match self.games[index].engine.state.outcome() {
            Outcome::Winner(p) if p == self.agent_seats[index] => self.agent_wins += 1,
            Outcome::Winner(_) => {}
            _ => self.draws += 1,
        }
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
    #[pyo3(signature = (
        num_envs, seed=0, max_day=60, fog=false, shaping=0.0, map_path=None,
        opponent=None, record=false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        num_envs: usize,
        seed: u64,
        max_day: u16,
        fog: bool,
        shaping: f32,
        map_path: Option<String>,
        opponent: Option<&str>,
        record: bool,
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
            opponents: Vec::new(),
            agent_seats: (0..num_envs).map(|i| (i % 2) as PlayerId).collect(),
            finished: 0,
            agent_wins: 0,
            draws: 0,
            rejected: 0,
            submitted: 0,
            recorders: if record {
                (0..num_envs).map(|_| Recorder::default()).collect()
            } else {
                Vec::new()
            },
        };
        for i in 0..num_envs {
            let state = env.board.new_state(fog);
            let engine = Engine::new(state, seed.wrapping_add(i as u64));
            let last_advantage = advantage(&engine.state, engine.state.current);
            env.games.push(Game { engine, last_advantage, steps: 0 });
            env.masks.push(ActionMasks::new());
        }
        if let Some(name) = opponent {
            env.opponents = (0..num_envs)
                .map(|i| make_teacher(name, seed.wrapping_add(0xA11CE + i as u64)))
                .collect::<PyResult<Vec<_>>>()?;
            for i in 0..num_envs {
                env.run_opponent(i);
                env.games[i].last_advantage =
                    advantage(&env.games[i].engine.state, env.agent_seats[i]);
            }
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
            if !self.opponents.is_empty() {
                self.opponents[i].reset(self.seed.wrapping_add(self.episodes));
                self.run_opponent(i);
            }
        }
        self.observe(py)
    }

    /// `(games finished, wins for [`VecEnv::agent_seat`], draws)`.
    ///
    /// Draws are almost all games that hit the day cap. Counting them apart
    /// from losses matters: a policy that never loses but never wins is a
    /// different failure from one that is simply beaten.
    ///
    /// Kept in self-play too, where it counts one nominated seat rather than
    /// one player. Two copies of the same policy should sit at half, so this
    /// doubles as a check that the seats really are being alternated -- and it
    /// is the rating when the two seats hold *different* checkpoints.
    #[getter]
    fn results(&self) -> (u64, u64, u64) {
        (self.finished, self.agent_wins, self.draws)
    }

    /// `(orders submitted, orders the engine refused)`. The second should stay
    /// at zero for anything sampling under the masks.
    #[getter]
    fn order_stats(&self) -> (u64, u64) {
        (self.submitted, self.rejected)
    }

    /// The last finished game in a slot, as JSON, or `None`.
    ///
    /// Needs `record=True`. Taking it clears it, so a caller polling every step
    /// collects each game exactly once; a game nobody collects before the slot
    /// finishes another is dropped. Feed the JSON to `tools/write_replay.py` to
    /// get a file AWBW's own replay viewers will open.
    fn take_replay(&mut self, index: usize) -> PyResult<Option<String>> {
        if self.recorders.is_empty() {
            return Err(PyValueError::new_err("construct VecEnv with record=True"));
        }
        if index >= self.recorders.len() {
            return Err(PyValueError::new_err(format!(
                "slot {index} of {}",
                self.recorders.len()
            )));
        }
        Ok(self.recorders[index]
            .take()
            .map(|log| serde_json::to_string(&log).unwrap_or_default()))
    }

    /// The seat the caller plays in each game.
    fn agent_seat<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<i64>> {
        let data: Vec<i64> = self.agent_seats.iter().map(|&s| s as i64).collect();
        data.into_pyarray(py)
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

        let versus = !self.opponents.is_empty();
        let (mut bad, mut sent) = (0u64, 0u64);
        for i in 0..self.games.len() {
            // With an opponent the caller only ever moves its own seat, so that
            // is whose side the reward is measured from; in self-play it is
            // whoever just moved.
            let scored_for = if versus {
                self.agent_seats[i]
            } else {
                self.games[i].engine.state.current
            };
            let code = ActionCode {
                source: sources[i],
                dest: dests[i],
                kind: kinds[i] as u8,
                param: params[i],
            };
            // An unmasked or stale selection forfeits the turn rather than
            // failing the whole batch; a policy sampling under the masks will
            // never land here.
            let decoded = decode(&self.games[i].engine.state, code);
            let action = decoded.unwrap_or(Action::EndTurn);
            actors[i] = self.games[i].engine.state.current as i64;
            // Read before applying: the path a unit walks does not survive it.
            let before = (!self.recorders.is_empty())
                .then(|| self.recorders[i].begin(&self.games[i].engine.state, action));

            let refused = {
                let game = &mut self.games[i];
                let refused = game.engine.apply(action).is_err();
                if refused {
                    let _ = game.engine.apply(Action::EndTurn);
                }
                game.steps += 1;
                refused
            };
            bad += u64::from(decoded.is_none() || refused);
            sent += 1;
            if let Some(before) = before {
                let played = if refused { Action::EndTurn } else { action };
                self.recorders[i].end(&self.games[i].engine, played, before);
            }
            self.run_opponent(i);

            let game = &mut self.games[i];
            let advantage_now = advantage(&game.engine.state, scored_for);
            rewards[i] = shaping * (advantage_now - game.last_advantage);
            game.last_advantage = advantage_now;

            if VecEnv::is_finished(&game.engine.state, max_day) {
                dones[i] = true;
                rewards[i] += match game.engine.state.outcome() {
                    Outcome::Winner(p) if p == scored_for => 1.0,
                    Outcome::Winner(_) => -1.0,
                    _ => 0.0,
                };
                self.score(i);
                if !self.recorders.is_empty() {
                    self.recorders[i].finish(&self.games[i].engine.state);
                }
                restarts.push(i);
            }
        }

        self.rejected += bad;
        self.submitted += sent;
        self.episodes += 1;
        for i in restarts {
            self.games[i] = self.new_game(i, self.episodes);
            if !self.recorders.is_empty() {
                // The finished log is already sealed and waiting; this drops
                // only the turn bookkeeping so the next game starts clean.
                self.recorders[i].clear();
            }
            if versus {
                self.opponents[i].reset(self.seed.wrapping_add(self.episodes));
                self.run_opponent(i);
            }
        }
        // Every game's advantage baseline follows the side the reward is for.
        for i in 0..self.games.len() {
            let seat = if versus {
                self.agent_seats[i]
            } else {
                self.games[i].engine.state.current
            };
            self.games[i].last_advantage = advantage(&self.games[i].engine.state, seat);
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
        "jakeman" => Box::new(JakeManBot::new(seed)),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown teacher {other:?}; expected greedy, jakeman, capturer or random"
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
        let inner = VecEnv::new(num_envs, seed, max_day, fog, 0.0, map_path, None, false)?;
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

    #[getter]
    fn end_turn_index(&self) -> u32 {
        self.inner.end_turn_index()
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
    masks: Vec<ActionMasks>,
    /// How far into its first game each slot starts.
    ///
    /// Without this every slot opens at turn 0, so the first hundred batches
    /// are a hundred views of day one — all builds and captures, no combat —
    /// and a small held-out set that keeps restarting never shows anything
    /// else. Only the *first* game is skipped into, so steady-state coverage
    /// of a game stays even.
    stagger: Vec<usize>,
    pending: Vec<usize>,
    want_map: Option<String>,
    shape: (u8, u8),
    obs_len: usize,
    sizes: [usize; 4],
    end_turn: u32,
    map_name: String,
    skip_powers: bool,
    skip_illegal: bool,
    /// Read each turn ahead so `source_targets` can answer. Costs a second load
    /// and replay of every turn, so it is off unless a trainer asks.
    lookahead: bool,
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
            return Some(Cursor::with_lookahead(verifier, self.lookahead));
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
                let Some(fresh) = cursor.as_mut() else { break };
                let skip = std::mem::take(&mut self.pending[slot]);
                for _ in 0..skip {
                    if fresh.finished() {
                        break;
                    }
                    fresh.advance();
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
        holdout=0.0,
        validation=false,
        lookahead=false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        replay_dir: &str,
        num_envs: usize,
        map_name: Option<&str>,
        seed: u64,
        skip_powers: bool,
        skip_illegal: bool,
        holdout: f64,
        validation: bool,
        lookahead: bool,
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

        // The split is by *game*, not by order. Two orders from the same turn
        // are nearly the same position, so splitting orders would leave the
        // validation set memorised rather than held out.
        if !(0.0..1.0).contains(&holdout) {
            return Err(PyValueError::new_err("holdout must be in 0.0..1.0"));
        }
        if holdout > 0.0 {
            let kept = ((files.len() as f64) * (1.0 - holdout)).round() as usize;
            let kept = kept.clamp(1, files.len() - 1);
            if validation {
                files.drain(..kept);
            } else {
                files.truncate(kept);
            }
        } else if validation {
            return Err(PyValueError::new_err(
                "validation=True needs holdout > 0.0",
            ));
        }

        // A typical game is about 380 orders, so this spreads the slots over a
        // whole one without needing to know its length in advance.
        let stagger: Vec<usize> = (0..num_envs)
            .map(|_| rng.roll_inclusive(360) as usize)
            .collect();

        let mut teacher = ReplayTeacher {
            files,
            next_file: 0,
            slots: (0..num_envs).map(|_| None).collect(),
            masks: (0..num_envs).map(|_| ActionMasks::new()).collect(),
            pending: stagger.clone(),
            stagger,
            want_map: map_name.map(str::to_string),
            shape: (0, 0),
            obs_len: 0,
            sizes: [0; 4],
            end_turn: 0,
            map_name: String::new(),
            skip_powers,
            skip_illegal,
            lookahead,
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

    /// Returns every slot to where it started.
    ///
    /// A validation pass has to score the *same* orders each time it runs, or a
    /// rising number cannot be told from an easier sample. Streaming teachers
    /// do not naturally do that, so this rewinds one.
    fn reset(&mut self) {
        self.next_file = 0;
        self.epochs = 0;
        self.pending.copy_from_slice(&self.stagger);
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
        for i in 0..self.slots.len() {
            self.refill(i);
        }
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

    /// Which tiles could act in each slot's position, plus the end-turn index.
    ///
    /// The same mask the policy plays under, so a prediction made here is made
    /// under play conditions — and so a uniform draw over it is a meaningful
    /// baseline rather than a draw over the whole board.
    fn source_mask<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<bool>> {
        let width = self.shape.0 as usize * self.shape.1 as usize + 1;
        let mut data = vec![false; self.slots.len() * width];
        let mut scratch = Vec::new();
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let Some(engine) = slot.as_mut().and_then(|c| c.engine_mut()) else {
                continue;
            };
            self.masks[i].source_mask(engine, &mut scratch);
            data[i * width..i * width + scratch.len()].copy_from_slice(&scratch);
        }
        Array2::from_shape_vec((self.slots.len(), width), data)
            .expect("mask buffer is rectangular")
            .into_pyarray(py)
    }

    /// Which tiles the human still acts from this turn — an order-invariant
    /// target for the source head.
    ///
    /// Ending the turn is excluded unless the human ended here. It is the last
    /// thing every turn does, so leaving it in would put it in every target
    /// set, and a policy could satisfy the loss by ending every turn at once.
    fn source_targets<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<bool>> {
        let width = self.shape.0 as usize * self.shape.1 as usize + 1;
        let end = self.end_turn;
        let mut data = vec![false; self.slots.len() * width];
        for (i, slot) in self.slots.iter().enumerate() {
            let Some(cursor) = slot.as_ref() else { continue };
            let remaining = cursor.remaining_sources();
            let row = &mut data[i * width..(i + 1) * width];
            match remaining.first() {
                // The human ends the turn here, so that is the only answer.
                Some(&first) if first == end => row[end as usize] = true,
                Some(_) => {
                    for &source in remaining {
                        if source != end && (source as usize) < width {
                            row[source as usize] = true;
                        }
                    }
                }
                None => {}
            }
        }
        Array2::from_shape_vec((self.slots.len(), width), data)
            .expect("target buffer is rectangular")
            .into_pyarray(py)
    }

    /// Which recorded turn each slot is walking. `-1` where a slot has no game.
    ///
    /// A caller that wants to reason about a *turn* rather than an order — the
    /// orders in one are largely interchangeable, so scoring them one at a time
    /// punishes a policy for picking a different but equally good sequence —
    /// needs to know where one turn ends and the next begins.
    fn turn_index<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<i64>> {
        let data: Vec<i64> = self
            .slots
            .iter()
            .map(|s| s.as_ref().map_or(-1, |c| c.turn_index() as i64))
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
