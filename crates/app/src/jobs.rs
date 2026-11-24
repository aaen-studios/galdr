//! The core's jobs as the app sees them: exports, conversions and compressions, and how far
//! along each is, from the core's events.

use std::path::PathBuf;

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Export,
    Convert,
    Compress,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Queued,
    Running,
    Finished,
    Failed,
    Canceled,
}

pub struct Job {
    pub id: u64,
    pub kind: Kind,
    /// What it's made from: a file's name, or the timeline.
    pub name: String,
    pub output: PathBuf,
    pub state: State,
    /// Seconds of the output written, and of the output in all once known.
    pub done: f64,
    pub duration: f64,
    pub error: Option<String>,
}

impl Job {
    pub fn active(&self) -> bool {
        matches!(self.state, State::Queued | State::Running)
    }

    pub fn progress(&self) -> f32 {
        if self.state == State::Finished {
            return 1.0;
        }
        (self.done / self.duration.max(0.001)).clamp(0.0, 1.0) as f32
    }

    pub fn output_name(&self) -> String {
        self.output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

#[derive(Default)]
pub struct Jobs {
    pub list: Vec<Job>,
    next: u64,
}

impl Jobs {
    /// An id for a request to the core, unique among them all.
    pub fn next_id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// A new job, queued. Gives its id, for the request.
    pub fn add(&mut self, kind: Kind, name: String, output: PathBuf, duration: f64) -> u64 {
        let id = self.next_id();
        self.list.push(Job { id, kind, name, output, state: State::Queued, done: 0.0, duration, error: None });
        id
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Job> {
        self.list.iter_mut().find(|j| j.id == id)
    }

    /// The newest job of a kind.
    pub fn latest(&self, kind: Kind) -> Option<&Job> {
        self.list.iter().rev().find(|j| j.kind == kind)
    }

    pub fn any_active(&self, kind: Kind) -> bool {
        self.list.iter().any(|j| j.kind == kind && j.active())
    }

    pub fn of(&self, kind: Kind) -> impl Iterator<Item = &Job> {
        self.list.iter().rev().filter(move |j| j.kind == kind)
    }

    /// Takes one of the core's events.
    pub fn apply(&mut self, v: &Value) {
        let Some(job) = v["id"].as_u64().and_then(|id| self.get_mut(id)) else { return };
        let state = match v["event"].as_str().unwrap_or("") {
            "queued" => State::Queued,
            "started" | "progress" => State::Running,
            "finished" => State::Finished,
            "failed" => State::Failed,
            "canceled" => State::Canceled,
            _ => return,
        };
        // A progress line can come after the end.
        if !job.active() {
            return;
        }
        job.state = state;
        if let Some(d) = v["duration"].as_f64() {
            job.duration = d;
        }
        if let Some(d) = v["done"].as_f64() {
            job.done = d;
        }
        if let Some(e) = v["error"].as_str() {
            job.error = Some(e.to_string());
        }
    }

    /// The core crashed: whatever it was doing has failed.
    pub fn crashed(&mut self, why: &str) {
        for j in self.list.iter_mut().filter(|j| j.active()) {
            j.state = State::Failed;
            j.error = Some(why.to_string());
        }
    }

    pub fn failed(&mut self, id: u64, why: &str) {
        if let Some(j) = self.get_mut(id) {
            j.state = State::Failed;
            j.error = Some(why.to_string());
        }
    }

    /// Finished and failed jobs off the list, keeping running ones.
    pub fn clear_done(&mut self, kind: Kind) {
        self.list.retain(|j| j.kind != kind || j.active());
    }

    /// Where a tool writes its output: next to the original, as `name-suffix.ext`, numbered if a
    /// file or a queued job has that name.
    pub fn output_for(&self, input: &std::path::Path, suffix: &str, ext: &str) -> PathBuf {
        let dir = input.parent().map(PathBuf::from).unwrap_or_default();
        let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "output".into());
        let taken = |p: &PathBuf| p.exists() || self.list.iter().any(|j| j.active() && &j.output == p);
        let mut path = dir.join(format!("{stem}-{suffix}.{ext}"));
        let mut n = 2;
        while taken(&path) {
            path = dir.join(format!("{stem}-{suffix}-{n}.{ext}"));
            n += 1;
        }
        path
    }
}
