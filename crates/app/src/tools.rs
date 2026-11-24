//! The tool pages in the rail: files in, a few choices, and jobs out. Convert and Compress run
//! ffmpeg through the core; Transcribe and Download say what they need.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, ObjectFit, PathPromptOptions, RenderImage, SharedString, Stateful,
    div, img, prelude::*, px, relative,
};
use serde_json::json;

use crate::Galdr;
use crate::assets::MONO;
use crate::jobs::{Job, Kind, State};
use crate::project::MediaItem;
use crate::theme::Theme;
use crate::view::{TOOLS, button, glyph, icon_button, outline_button, short_time};

/// Convert's formats: the core's name, what it's called, and the file extension.
pub const FORMATS: [(&str, &str, &str); 7] = [
    ("mp4", "MP4", "mp4"),
    ("h265", "H.265", "mp4"),
    ("webm", "WebM", "webm"),
    ("prores", "ProRes", "mov"),
    ("gif", "GIF", "gif"),
    ("mp3", "MP3", "mp3"),
    ("wav", "WAV", "wav"),
];
pub const QUALITIES: [(&str, &str); 3] = [("high", "High"), ("balanced", "Balanced"), ("small", "Small")];
pub const FITS: [(f64, &str); 5] = [(0.0, "Any size"), (10.0, "10 MB"), (25.0, "25 MB"), (50.0, "50 MB"), (100.0, "100 MB")];
pub const HEIGHTS: [(u32, &str); 4] = [(0, "Original"), (1080, "1080p"), (720, "720p"), (480, "480p")];

/// Which tool page: the index in `TOOLS`.
pub const CONVERT: usize = 1;
pub const COMPRESS: usize = 2;

pub struct ToolFile {
    pub path: PathBuf,
    pub name: String,
    pub info: Option<media::Info>,
    pub error: Option<String>,
    pub thumb: Option<Arc<RenderImage>>,
}

pub struct Tools {
    pub convert: Vec<ToolFile>,
    pub compress: Vec<ToolFile>,
    pub format: usize,
    pub quality: usize,
    pub fit: usize,
    pub height: usize,
}

impl Default for Tools {
    fn default() -> Self {
        Self { convert: Vec::new(), compress: Vec::new(), format: 0, quality: 1, fit: 0, height: 0 }
    }
}

impl Tools {
    fn files(&self, tool: usize) -> &Vec<ToolFile> {
        if tool == CONVERT { &self.convert } else { &self.compress }
    }

    fn files_mut(&mut self, tool: usize) -> &mut Vec<ToolFile> {
        if tool == CONVERT { &mut self.convert } else { &mut self.compress }
    }
}

fn kind_of(tool: usize) -> Kind {
    if tool == CONVERT { Kind::Convert } else { Kind::Compress }
}

fn meta(info: &media::Info) -> String {
    format!("{}×{} · {}p", info.width, info.height, (info.fps * 100.0).round() / 100.0)
}

/// A choice among a few, in loom's segmented control.
fn segments<T: Copy + 'static>(
    id: &'static str,
    t: &Theme,
    options: &[(T, &'static str)],
    chosen: usize,
    cx: &mut Context<Galdr>,
    pick: fn(&mut Galdr, usize),
) -> Div {
    let hover = t.ink;
    div().flex().flex_wrap().p(px(2.)).gap(px(2.)).rounded(px(8.)).bg(t.raised).children(options.iter().enumerate().map(
        |(i, (_, label))| {
            let on = i == chosen;
            div()
                .id((id, i))
                .h(px(26.))
                .px(px(11.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_size(px(12.5))
                .cursor_pointer()
                .when(on, |d| d.bg(t.bg).border_1().border_color(t.line_strong).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.text_color(t.ink_3).hover(move |s| s.text_color(hover)))
                .child(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    pick(this, i);
                    cx.notify();
                }))
        },
    ))
}

/// An option: its name on the left, its control on the right.
fn option_row(t: &Theme, title: &'static str, control: impl IntoElement) -> Div {
    div()
        .min_h(px(50.))
        .py(px(8.))
        .flex()
        .items_center()
        .gap(px(16.))
        .border_t_1()
        .border_color(t.line)
        .child(div().w(px(96.)).flex_none().text_color(t.ink_2).child(title))
        .child(div().flex_1().min_w_0().flex().justify_end().child(control))
}

fn card(t: &Theme) -> Div {
    div().p(px(14.)).rounded(px(14.)).border_1().border_color(t.line).bg(t.bg).flex().flex_col()
}

fn link(id: impl Into<ElementId>, label: &'static str, t: &Theme) -> Stateful<Div> {
    let hover = t.ink;
    div()
        .id(id)
        .text_size(px(12.))
        .text_color(t.ink_3)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover))
        .child(label)
}

impl Galdr {
    /// Files dropped on the window: onto a tool's list on its page, or into the media.
    pub fn dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        match self.page {
            crate::Page::Tool(i) if i == CONVERT || i == COMPRESS => self.add_tool_files(i, paths, cx),
            _ => self.import(paths, cx),
        }
    }

    pub fn add_tool_files(&mut self, tool: usize, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            if self.tools.files(tool).iter().any(|f| f.path == path) {
                continue;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            self.tools.files_mut(tool).push(ToolFile { path: path.clone(), name, info: None, error: None, thumb: None });
            cx.spawn(async move |this, cx| {
                let p = path.clone();
                let read = cx
                    .background_executor()
                    .spawn(async move {
                        let (info, frame) = media::thumbnail(&p, 1.0, 120)?;
                        anyhow::Ok((info, frame))
                    })
                    .await;
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    let Some(f) = app.tools.files_mut(tool).iter_mut().find(|f| f.path == path) else { return };
                    match read {
                        Ok((info, frame)) => {
                            f.info = Some(info);
                            f.thumb = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
                                .map(|b| Arc::new(RenderImage::new([image::Frame::new(b)])));
                        }
                        // Sound-only files have no picture; ffmpeg may still take them.
                        Err(e) => f.error = Some(format!("{e:#}")),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    fn prompt_tool_files(&mut self, tool: usize, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |app: &mut Galdr, cx| app.add_tool_files(tool, paths, cx));
            }
        })
        .detach();
    }

    pub fn start_convert(&mut self, input: &Path, output: PathBuf, format: &str) -> u64 {
        let name = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let id = self.jobs.add(Kind::Convert, name, output.clone(), 0.0);
        self.send_job(
            id,
            json!({
                "cmd": "convert", "id": id, "input": input.display().to_string(),
                "output": output.display().to_string(), "format": format,
            }),
        );
        id
    }

    pub fn start_compress(&mut self, input: &Path, output: PathBuf, quality: &str, max_height: u32, target_mb: f64) -> u64 {
        let name = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let id = self.jobs.add(Kind::Compress, name, output.clone(), 0.0);
        self.send_job(
            id,
            json!({
                "cmd": "compress", "id": id, "input": input.display().to_string(),
                "output": output.display().to_string(), "quality": quality,
                "max_height": max_height, "target_mb": target_mb,
            }),
        );
        id
    }

    /// Starts the tool on every file in its list, and empties the list.
    fn run_tool(&mut self, tool: usize) {
        let files = std::mem::take(self.tools.files_mut(tool));
        for f in files {
            if tool == CONVERT {
                let (format, _, ext) = FORMATS[self.tools.format];
                let output = self.jobs.output_for(&f.path, "converted", ext);
                self.start_convert(&f.path, output, format);
            } else {
                let (quality, _) = QUALITIES[self.tools.quality];
                let (h, _) = HEIGHTS[self.tools.height];
                // Only ever down.
                let max_height = if f.info.as_ref().is_some_and(|i| h > 0 && i.height > h) { h } else { 0 };
                let output = self.jobs.output_for(&f.path, "compressed", "mp4");
                self.start_compress(&f.path, output, quality, max_height, FITS[self.tools.fit].0);
            }
        }
    }

    pub fn render_tool_page(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        if i != CONVERT && i != COMPRESS {
            let (name, title) = TOOLS[i];
            let needs = if name == "transcribe" { "whisper.cpp" } else { "yt-dlp" };
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(glyph(name, 28., t.tools[i + 1]))
                .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_color(t.ink_3).child(SharedString::from(format!("Not built yet. It will use {needs}."))))
                .into_any_element();
        }
        let (icon_name, title) = TOOLS[i];
        let count = self.tools.files(i).len();
        let verb = if i == CONVERT { "Convert" } else { "Compress" };
        let go = match count {
            0 => verb.to_string(),
            1 => format!("{verb} 1 file"),
            n => format!("{verb} {n} files"),
        };
        let example = self.tools.files(i).first().map(|f| {
            let stem = f.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if i == CONVERT {
                format!("{stem}-converted.{}", FORMATS[self.tools.format].2)
            } else {
                format!("{stem}-compressed.mp4")
            }
        });
        let options = if i == CONVERT {
            card(&t)
                .pt(px(4.))
                .pb(px(4.))
                .child(
                    option_row(
                        &t,
                        "Format",
                        segments(
                            "format",
                            &t,
                            &FORMATS.map(|(k, l, _)| (k, l)),
                            self.tools.format,
                            cx,
                            |a, i| a.tools.format = i,
                        ),
                    )
                    .border_t_0(),
                )
        } else {
            card(&t)
                .pt(px(4.))
                .pb(px(4.))
                .child(
                    option_row(
                        &t,
                        "Quality",
                        segments("quality", &t, &QUALITIES, self.tools.quality, cx, |a, i| a.tools.quality = i),
                    )
                    .border_t_0(),
                )
                .child(option_row(&t, "Fit in", segments("fit", &t, &FITS, self.tools.fit, cx, |a, i| a.tools.fit = i)))
                .child(option_row(
                    &t,
                    "Height",
                    segments("height", &t, &HEIGHTS, self.tools.height, cx, |a, i| a.tools.height = i),
                ))
        }
        .child(option_row(
            &t,
            "Saved",
            div()
                .flex()
                .gap(px(8.))
                .min_w_0()
                .child(div().text_color(t.ink_3).child("next to the original, as"))
                .child(
                    div()
                        .truncate()
                        .font_family(MONO)
                        .text_size(px(12.))
                        .child(SharedString::from(example.unwrap_or_else(|| {
                            if i == CONVERT { "name-converted".into() } else { "name-compressed.mp4".into() }
                        }))),
                ),
        ));

        let jobs: Vec<AnyElement> = self.jobs.of(kind_of(i)).take(30).map(|j| self.render_job_row(j, cx)).collect();
        let has_done = self.jobs.of(kind_of(i)).any(|j| !j.active());

        div()
            .id(("tool-page", i))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .px(px(24.))
                    .pt(px(36.))
                    .pb(px(48.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .pb(px(4.))
                            .child(glyph(icon_name, 22., t.tools[i + 1]))
                            .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child(title)),
                    )
                    .child(self.render_tool_files(i, cx))
                    .child(options)
                    .child(
                        div().flex().child(button(("run", i), go, &t, count > 0).when(count > 0, |d| {
                            d.on_click(cx.listener(move |this, _, _, cx| {
                                this.run_tool(i);
                                cx.notify();
                            }))
                        })),
                    )
                    .when(!jobs.is_empty(), |d| {
                        d.child(
                            div()
                                .pt(px(14.))
                                .flex()
                                .items_center()
                                .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("Jobs"))
                                .when(has_done, |d| {
                                    d.child(link(("clear", i), "Clear finished", &t).on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.jobs.clear_done(kind_of(i));
                                            cx.notify();
                                        },
                                    )))
                                }),
                        )
                        .child(div().flex().flex_col().children(jobs))
                    }),
            )
            .into_any_element()
    }

    fn render_tool_files(&self, tool: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let files = self.tools.files(tool);
        // The media that isn't on the list yet, to add with a click.
        let from_media: Vec<&MediaItem> =
            self.project.media.iter().filter(|m| !files.iter().any(|f| f.path == m.path)).take(8).collect();
        let body: AnyElement = if files.is_empty() {
            div()
                .mt(px(10.))
                .py(px(26.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .rounded(px(10.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .child(glyph("upload", 20., t.ink_3))
                .child(div().text_size(px(12.5)).text_color(t.ink_3).child("Drop files here"))
                .into_any_element()
        } else {
            div()
                .mt(px(8.))
                .flex()
                .flex_col()
                .children(files.iter().enumerate().map(|(k, f)| {
                    let (meta_text, color): (SharedString, _) = match (&f.info, &f.error) {
                        (Some(i), _) => (meta(i).into(), t.ink_3),
                        (None, Some(_)) => ("No picture".into(), t.ink_3),
                        (None, None) => ("Reading…".into(), t.ink_3),
                    };
                    let path = f.path.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(11.))
                        .py(px(6.))
                        .px(px(4.))
                        .child(
                            div()
                                .w(px(58.))
                                .h(px(33.))
                                .flex_none()
                                .rounded(px(5.))
                                .overflow_hidden()
                                .bg(t.raised_2)
                                .children(f.thumb.clone().map(|i| img(i).size_full().object_fit(ObjectFit::Cover))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().child(SharedString::from(f.name.clone())))
                                .child(div().truncate().text_size(px(11.5)).text_color(color).child(meta_text)),
                        )
                        .children(f.info.as_ref().map(|i| {
                            div()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .text_color(t.ink_3)
                                .child(SharedString::from(short_time(i.duration)))
                        }))
                        .child(icon_button(("remove", tool * 1000 + k), "x", &t).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.tools.files_mut(tool).retain(|f| f.path != path);
                                cx.notify();
                            },
                        )))
                        .into_any_element()
                }))
                .into_any_element()
        };
        card(&t)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Files"))
                    .when(!files.is_empty(), |d| {
                        d.child(
                            div().text_color(t.ink_3).text_size(px(12.)).child(SharedString::from(files.len().to_string())),
                        )
                    })
                    .child(div().flex_1())
                    .child(outline_button(("add-files", tool), "Add files", &t, true).on_click(cx.listener(
                        move |this, _, _, cx| this.prompt_tool_files(tool, cx),
                    ))),
            )
            .child(body)
            .when(!from_media.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(10.))
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(6.))
                        .child(div().text_size(px(12.)).text_color(t.ink_3).mr(px(2.)).child("From media"))
                        .children(from_media.into_iter().map(|m| {
                            let path = m.path.clone();
                            let hover = t.active;
                            div()
                                .id(("from-media", m.id as usize))
                                .h(px(26.))
                                .px(px(9.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .rounded(px(7.))
                                .bg(t.raised)
                                .text_size(px(12.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .child(glyph("plus", 13., t.ink_2))
                                .child(SharedString::from(m.name.clone()))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.add_tool_files(tool, vec![path.clone()], cx);
                                    cx.notify();
                                }))
                        })),
                )
            })
    }

    fn render_job_row(&self, j: &Job, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let id = j.id;
        let output = j.output.clone();
        let f = j.progress();
        let status: AnyElement = match j.state {
            State::Finished => div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().text_color(t.ok).child("Done"))
                .child(link(("show", id as usize), "Show ›", &t).on_click(move |_, _, _| Galdr::reveal(&output)))
                .into_any_element(),
            State::Failed => div().text_color(t.bad).child("Failed").into_any_element(),
            State::Canceled => div().text_color(t.ink_3).child("Canceled").into_any_element(),
            State::Queued | State::Running => div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(120.))
                        .h(px(3.))
                        .rounded_full()
                        .bg(t.raised_2)
                        .overflow_hidden()
                        .child(div().h_full().w(relative(f)).bg(t.ink)),
                )
                .child(
                    div()
                        .w(px(34.))
                        .flex()
                        .justify_end()
                        .font_family(MONO)
                        .text_size(px(11.5))
                        .text_color(t.ink_3)
                        .child(SharedString::from(if j.state == State::Queued {
                            "–".to_string()
                        } else {
                            format!("{:.0}%", f * 100.0)
                        })),
                )
                .child(icon_button(("cancel", id as usize), "x", &t).on_click(cx.listener(move |this, _, _, cx| {
                    this.cancel_job(id);
                    cx.notify();
                })))
                .into_any_element(),
        };
        div()
            .py(px(9.))
            .border_b_1()
            .border_color(t.line)
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .min_h(px(30.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().truncate().child(SharedString::from(j.name.clone())))
                            .child(div().flex_none().text_color(t.ink_3).child("→"))
                            .child(
                                div()
                                    .truncate()
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .text_color(t.ink_2)
                                    .child(SharedString::from(j.output_name())),
                            ),
                    )
                    .child(status),
            )
            .children(j.error.as_ref().filter(|_| j.state == State::Failed).map(|e| {
                div().text_size(px(11.5)).text_color(t.ink_3).child(SharedString::from(e.lines().last().unwrap_or("").to_string()))
            }))
            .into_any_element()
    }
}
