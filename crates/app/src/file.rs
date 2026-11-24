//! Project files: saving and opening `.galdr` files through the core, the project's name, and
//! whether it has changed since it was saved.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use gpui::{Context, PathPromptOptions, PromptButton, PromptLevel, Window};
use serde_json::{Value, json};

use crate::Galdr;
use crate::project::Clip;

/// What to do once the project is saved, or the user says not to save it.
#[derive(Clone, Copy, Debug)]
pub enum Then {
    Quit,
    New,
    Open,
}

/// A request to the core that the app waits on.
pub enum Pending {
    Save(PathBuf),
    Open(PathBuf),
}

/// What the project was when it was last saved or opened, to tell whether it changed since.
#[derive(Default)]
pub struct Saved {
    clips: Vec<Clip>,
    media: Vec<PathBuf>,
}

/// How long a notice stays in the title bar.
pub const NOTICE_SECS: f64 = 6.0;

impl Galdr {
    pub fn project_name(&self) -> String {
        self.project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    /// Whether there's anything a save would keep that isn't saved.
    pub fn dirty(&self) -> bool {
        let media: Vec<&PathBuf> = self.project.media.iter().map(|m| &m.path).collect();
        self.saved.clips != self.project.clips || self.saved.media.iter().collect::<Vec<_>>() != media
    }

    fn mark_saved(&mut self) {
        self.saved = Saved {
            clips: self.project.clips.clone(),
            media: self.project.media.iter().map(|m| m.path.clone()).collect(),
        };
    }

    /// A line in the title bar for a few seconds.
    pub fn notify_user(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        match self.project_path.clone() {
            Some(path) => self.write_project(path),
            None => self.save_as(cx),
        }
    }

    pub fn save_as(&mut self, cx: &mut Context<Self>) {
        let dir = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from))
            .or_else(|| self.project.media.first().and_then(|m| m.path.parent().map(PathBuf::from)))
            .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Videos")))
            .unwrap_or_else(std::env::temp_dir);
        let name = format!("{}.galdr", self.project_name());
        let path = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let chosen = match path.await {
                Ok(Ok(Some(path))) => Some(if path.extension().is_none() { path.with_extension("galdr") } else { path }),
                _ => None,
            };
            let _ = this.update(cx, |app: &mut Galdr, cx| {
                match chosen {
                    Some(path) => app.write_project(path),
                    // Not saved after all, so not closing either.
                    None => app.after_save = None,
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks the core to write the project to `path`.
    pub fn write_project(&mut self, path: PathBuf) {
        let project = json!({
            "galdr": 1,
            "media": self.project.media.iter().map(|m| json!({ "id": m.id, "path": m.path.display().to_string() })).collect::<Vec<_>>(),
            "clips": self.project.clips.iter().map(|c| json!({
                "media": c.media, "source_in": c.source_in, "source_out": c.source_out,
            })).collect::<Vec<_>>(),
            "playhead": self.preview.playhead,
        });
        let id = self.jobs.next_id();
        let request = json!({ "cmd": "save", "id": id, "path": path.display().to_string(), "project": project });
        if self.core.send(&request) {
            self.pending.insert(id, Pending::Save(path));
        } else {
            self.notify_user("Couldn't save: the core isn't running");
        }
    }

    pub fn prompt_open(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    app.open_project(path);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Asks the core to read the project at `path`; it opens when the core answers.
    pub fn open_project(&mut self, path: PathBuf) {
        let id = self.jobs.next_id();
        let request = json!({ "cmd": "open", "id": id, "path": path.display().to_string() });
        if self.core.send(&request) {
            self.pending.insert(id, Pending::Open(path));
        } else {
            self.notify_user("Couldn't open: the core isn't running");
        }
    }

    /// The window is asked to close: yes if nothing would be lost, and otherwise ask whether to
    /// save first, and quit once that's answered.
    pub fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.closing || !self.dirty() || self.auto.is_some() || self.snapshot.is_some() {
            return true;
        }
        self.guard(Then::Quit, window, cx);
        false
    }

    /// Does `then`, which drops the project, once it's saved or the user says not to save it.
    pub fn guard(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty() {
            self.proceed(then, cx);
            return;
        }
        let message = format!("Save the changes to {}?", self.project_name());
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            None,
            &[PromptButton::Ok("Save".into()), PromptButton::Other("Don't save".into()), PromptButton::Cancel("Cancel".into())],
            cx,
        );
        cx.spawn(async move |this, cx| {
            let Ok(answer) = answer.await else { return };
            let _ = this.update(cx, |app: &mut Galdr, cx| {
                match answer {
                    0 => {
                        app.after_save = Some(then);
                        app.save(cx);
                    }
                    1 => app.proceed(then, cx),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn proceed(&mut self, then: Then, cx: &mut Context<Self>) {
        match then {
            Then::Quit => {
                self.closing = true;
                cx.quit();
            }
            Then::New => self.new_project(),
            Then::Open => self.prompt_open(cx),
        }
    }

    /// Starts again with nothing.
    pub fn new_project(&mut self) {
        self.clear_project();
        self.project_path = None;
        self.mark_saved();
    }

    fn clear_project(&mut self) {
        self.preview.pause(&self.project);
        self.project.reset();
        self.thumbnails.clear();
        self.strips.clear();
        self.waveforms.clear();
        self.selected = None;
        self.selected_media = None;
        self.fit = true;
        self.edited();
        self.preview.seek(&self.project, 0.0, false);
    }

    /// An answer from the core to a save or an open. False if it wasn't one.
    pub fn core_reply(&mut self, v: &Value, cx: &mut Context<Self>) -> bool {
        let Some(id) = v["id"].as_u64() else { return false };
        let Some(pending) = self.pending.remove(&id) else { return false };
        let failed = v["event"] == "failed";
        let error = v["error"].as_str().unwrap_or("").to_string();
        match pending {
            Pending::Save(path) => {
                if failed {
                    self.after_save = None;
                    self.notify_user(format!("Couldn't save: {error}"));
                } else {
                    self.project_path = Some(path);
                    self.mark_saved();
                    self.notify_user("Saved");
                    if let Some(then) = self.after_save.take() {
                        self.proceed(then, cx);
                    }
                }
            }
            Pending::Open(path) => {
                if failed {
                    self.notify_user(format!("Couldn't open {}: {error}", path.display()));
                } else {
                    self.opened(path, v, cx);
                }
            }
        }
        true
    }

    fn opened(&mut self, path: PathBuf, v: &Value, cx: &mut Context<Self>) {
        self.clear_project();
        let p = &v["project"];
        // The file's media ids, to this project's.
        let mut ids = HashMap::new();
        for m in p["media"].as_array().into_iter().flatten() {
            let (Some(old), Some(file)) = (m["id"].as_u64(), m["path"].as_str()) else { continue };
            let id = self.import_one(PathBuf::from(file), cx);
            ids.insert(old, id);
        }
        for c in p["clips"].as_array().into_iter().flatten() {
            let (Some(media), Some(a), Some(b)) = (c["media"].as_u64(), c["source_in"].as_f64(), c["source_out"].as_f64())
            else {
                continue;
            };
            if let Some(&m) = ids.get(&media)
                && b > a
            {
                self.project.push_clip(m, a, b);
            }
        }
        self.edited();
        let playhead = p["playhead"].as_f64().unwrap_or(0.0);
        self.preview.seek(&self.project, playhead, false);
        self.project_path = Some(path);
        self.mark_saved();
        let missing = v["missing"].as_array().map_or(0, |m| m.len());
        if missing > 0 {
            self.notify_user(if missing == 1 { "1 file is missing".to_string() } else { format!("{missing} files are missing") });
        }
    }
}
