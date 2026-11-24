//! What's being edited: the imported media, and one magnetic video track of clips. Clips sit end
//! to end, so a clip's place on the timeline is the sum of the clips before it.

use std::path::PathBuf;

pub type MediaId = u64;
pub type ClipId = u64;

pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub name: String,
    /// Known once the file has been read.
    pub info: Option<media::Info>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: ClipId,
    pub media: MediaId,
    /// The part of the source file used, in seconds into it.
    pub source_in: f64,
    pub source_out: f64,
}

impl Clip {
    pub fn length(&self) -> f64 {
        self.source_out - self.source_in
    }
}

/// The shortest a clip can be trimmed to.
pub const MIN_CLIP: f64 = 0.1;

/// How many edits back undo goes.
const HISTORY: usize = 200;

#[derive(Default)]
pub struct Project {
    pub media: Vec<MediaItem>,
    pub clips: Vec<Clip>,
    next_id: u64,
    /// The track as it was before each edit, and as it was before each undo.
    undo: Vec<Vec<Clip>>,
    redo: Vec<Vec<Clip>>,
}

/// Where a timeline time falls: which clip, and how far into its source.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub index: usize,
    pub clip: ClipId,
    pub media: MediaId,
    pub source_t: f64,
    /// Where the clip starts on the timeline.
    pub clip_start: f64,
}

impl Project {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn add_media(&mut self, path: PathBuf) -> MediaId {
        let id = self.id();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.media.push(MediaItem { id, path, name, info: None, error: None });
        id
    }

    pub fn media(&self, id: MediaId) -> Option<&MediaItem> {
        self.media.iter().find(|m| m.id == id)
    }

    pub fn media_mut(&mut self, id: MediaId) -> Option<&mut MediaItem> {
        self.media.iter_mut().find(|m| m.id == id)
    }

    /// Empties the project for another. Ids carry on from where they were, so nothing new is
    /// mistaken for something from before, such as frames cached by media id.
    pub fn reset(&mut self) {
        self.media.clear();
        self.clips.clear();
        self.undo.clear();
        self.redo.clear();
    }

    /// Puts a clip at the end of the track as it is, as when opening a project.
    pub fn push_clip(&mut self, media: MediaId, source_in: f64, source_out: f64) -> ClipId {
        let id = self.id();
        self.clips.push(Clip { id, media, source_in, source_out });
        id
    }

    /// Remembers the track as it is, before an edit.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.clips.clone());
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Forgets the last checkpoint if nothing changed since, as when a drag ends where it began.
    pub fn settle(&mut self) {
        if self.undo.last() == Some(&self.clips) {
            self.undo.pop();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(clips) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.clips, clips));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(clips) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.clips, clips));
        true
    }

    /// Puts the whole of a file in the track at position `at`.
    pub fn insert(&mut self, media: MediaId, at: usize) -> Option<ClipId> {
        let duration = self.media(media)?.info.as_ref()?.duration;
        let id = self.id();
        self.clips.insert(at.min(self.clips.len()), Clip { id, media, source_in: 0.0, source_out: duration });
        Some(id)
    }

    /// Adds the whole of a file at the end of the track.
    pub fn append(&mut self, media: MediaId) -> Option<ClipId> {
        let duration = self.media(media)?.info.as_ref()?.duration;
        let id = self.id();
        self.clips.push(Clip { id, media, source_in: 0.0, source_out: duration });
        Some(id)
    }

    pub fn duration(&self) -> f64 {
        self.clips.iter().map(Clip::length).sum()
    }

    pub fn clip_start(&self, index: usize) -> f64 {
        self.clips[..index].iter().map(Clip::length).sum()
    }

    pub fn index_of(&self, clip: ClipId) -> Option<usize> {
        self.clips.iter().position(|c| c.id == clip)
    }

    /// The clip under timeline time `t`. Past the end, the last frame of the last clip.
    pub fn place(&self, t: f64) -> Option<Place> {
        let mut start = 0.0;
        for (index, c) in self.clips.iter().enumerate() {
            let end = start + c.length();
            if t < end || index == self.clips.len() - 1 {
                let into = (t - start).clamp(0.0, c.length());
                return Some(Place { index, clip: c.id, media: c.media, source_t: c.source_in + into, clip_start: start });
            }
            start = end;
        }
        None
    }

    /// Splits the clip under `t` in two there. Gives the new right-hand clip.
    pub fn split(&mut self, t: f64) -> Option<ClipId> {
        let p = self.place(t)?;
        let clip = &self.clips[p.index];
        if p.source_t - clip.source_in < MIN_CLIP || clip.source_out - p.source_t < MIN_CLIP {
            return None;
        }
        let right = Clip { id: 0, media: clip.media, source_in: p.source_t, source_out: clip.source_out };
        self.clips[p.index].source_out = p.source_t;
        let id = self.id();
        self.clips.insert(p.index + 1, Clip { id, ..right });
        Some(id)
    }

    pub fn remove(&mut self, clip: ClipId) -> bool {
        match self.index_of(clip) {
            Some(i) => {
                self.clips.remove(i);
                true
            }
            None => false,
        }
    }

    /// Moves a clip's in point to `source_t`, kept within the file and the clip's out point.
    pub fn trim_in(&mut self, clip: ClipId, source_t: f64) {
        if let Some(i) = self.index_of(clip) {
            let c = &mut self.clips[i];
            c.source_in = source_t.clamp(0.0, c.source_out - MIN_CLIP);
        }
    }

    /// Moves a clip's out point to `source_t`, kept within the file and the clip's in point.
    pub fn trim_out(&mut self, clip: ClipId, source_t: f64) {
        let Some(i) = self.index_of(clip) else { return };
        let duration = self.media(self.clips[i].media).and_then(|m| m.info.as_ref()).map(|i| i.duration);
        let c = &mut self.clips[i];
        c.source_out = source_t.clamp(c.source_in + MIN_CLIP, duration.unwrap_or(f64::MAX));
    }

    /// Moves a clip to position `to` in the track.
    pub fn move_clip(&mut self, clip: ClipId, to: usize) {
        if let Some(from) = self.index_of(clip) {
            let c = self.clips.remove(from);
            let to = to.min(self.clips.len());
            self.clips.insert(to, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (Project, MediaId) {
        let mut p = Project::default();
        let m = p.add_media(PathBuf::from("a.mp4"));
        p.media_mut(m).unwrap().info = Some(media::Info {
            width: 1920,
            height: 1080,
            fps: 30.0,
            duration: 10.0,
            has_audio: true,
            codec: "h264".into(),
        });
        (p, m)
    }

    #[test]
    fn split_and_place() {
        let (mut p, m) = project();
        p.append(m);
        p.append(m);
        assert_eq!(p.duration(), 20.0);
        let right = p.split(4.0).unwrap();
        assert_eq!(p.clips.len(), 3);
        assert_eq!(p.clips[0].source_out, 4.0);
        assert_eq!(p.clips[1].id, right);
        assert_eq!(p.clips[1].source_in, 4.0);
        let at = p.place(12.0).unwrap();
        assert_eq!((at.index, at.source_t), (2, 2.0));
        assert_eq!(p.place(25.0).unwrap().source_t, 10.0); // past the end: the last frame
        assert!(p.split(4.05).is_none()); // too close to the cut
    }

    #[test]
    fn trim_remove_move() {
        let (mut p, m) = project();
        let a = p.append(m).unwrap();
        let b = p.append(m).unwrap();
        p.trim_in(a, 2.0);
        p.trim_out(a, 50.0); // clamped to the file
        assert_eq!((p.clips[0].source_in, p.clips[0].source_out), (2.0, 10.0));
        p.trim_in(a, 9.99); // can't pass the out point
        assert!((p.clips[0].length() - MIN_CLIP).abs() < 1e-9);
        p.move_clip(b, 0);
        assert_eq!(p.clips[0].id, b);
        assert!(p.remove(a));
        assert_eq!(p.clips.len(), 1);
    }

    #[test]
    fn insert_and_open() {
        let (mut p, m) = project();
        let a = p.append(m).unwrap();
        let b = p.insert(m, 0).unwrap();
        let c = p.insert(m, 99).unwrap(); // past the end: at the end
        assert_eq!(p.clips.iter().map(|c| c.id).collect::<Vec<_>>(), vec![b, a, c]);
        // Ids carry on after a reset, so nothing new shares an id with something before.
        p.reset();
        assert!(p.clips.is_empty() && p.media.is_empty() && !p.can_undo());
        let m2 = p.add_media(PathBuf::from("b.mp4"));
        assert!(m2 > c);
        let d = p.push_clip(m2, 1.0, 2.0);
        assert_eq!((p.clips[0].id, p.clips[0].length()), (d, 1.0));
    }

    #[test]
    fn undo_redo() {
        let (mut p, m) = project();
        p.checkpoint();
        p.append(m);
        p.checkpoint();
        p.split(4.0);
        assert_eq!(p.clips.len(), 2);
        assert!(p.undo());
        assert_eq!(p.clips.len(), 1);
        assert!(p.undo());
        assert!(p.clips.is_empty());
        assert!(!p.undo());
        assert!(p.redo());
        assert!(p.redo());
        assert_eq!(p.clips.len(), 2);
        assert!(!p.redo());
        // A drag that ends where it began leaves nothing to undo.
        p.checkpoint();
        p.settle();
        assert!(p.undo());
        assert_eq!(p.clips.len(), 1);
        // A new edit drops what could be redone.
        p.checkpoint();
        p.split(2.0);
        assert!(!p.can_redo());
    }
}
