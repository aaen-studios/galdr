//! What the window shows, in loom's design language: the rail of tools down the left, galdr's
//! own title bar, the media list, the viewer and its transport, the clip card, and the timeline
//! in its well. Panels collapse and resize, and the viewer can take the whole window.

use std::sync::Arc;

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, DragMoveEvent, ElementId, ExternalPaths, FontWeight, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, RenderImage, Rgba, ScrollEvent, SharedString, Stateful, Svg, Window,
    WindowControlArea, canvas, deferred, div, fill, img, point, prelude::*, px, relative, rgb, size, svg,
};

use crate::assets::{FONT, MONO, icon};
use crate::jobs::{Kind, State};
use crate::preview::Mode;
use crate::project::{MediaId, MediaItem};
use crate::theme::{self, Theme};
use crate::{Divider, Drag, Galdr, LANE_PAD, PEAKS_PER_SECOND, Page, RULER_H, TOOLS_H, TRACK_TOP, auto, snapshot, timecode};

const RAIL_W: f32 = 60.0;
const BAR_H: f32 = 50.0;
const HEADS_W: f32 = 52.0;
/// The tools in the rail after Edit: icon and name.
pub(crate) const TOOLS: [(&str, &str); 4] =
    [("transcribe", "Transcribe"), ("convert", "Convert"), ("compress", "Compress"), ("download", "Download")];

/// A file being dragged from the media to the track, and how it looks on the way.
#[derive(Clone)]
pub struct MediaDrag {
    pub id: MediaId,
    name: SharedString,
    thumb: Option<Arc<RenderImage>>,
    theme: Theme,
}

impl Render for MediaDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(5.))
            .pr(px(10.))
            .rounded(px(8.))
            .bg(t.bg)
            .border_1()
            .border_color(t.line_strong)
            .shadow_md()
            .font_family(FONT)
            .text_size(px(12.5))
            .text_color(t.ink)
            .child(
                div()
                    .w(px(48.))
                    .h(px(27.))
                    .rounded(px(4.))
                    .overflow_hidden()
                    .bg(t.raised_2)
                    .children(self.thumb.clone().map(|i| img(i).size_full().object_fit(ObjectFit::Cover))),
            )
            .child(self.name.clone())
    }
}

pub(crate) fn glyph(name: &'static str, size: f32, color: Rgba) -> Svg {
    svg().path(icon(name)).size(px(size)).flex_none().text_color(color)
}

pub(crate) fn icon_button(id: impl Into<ElementId>, name: &'static str, t: &Theme) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id)
        .size(px(30.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(glyph(name, 17., t.ink_3))
}

pub(crate) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, t: &Theme, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.))
        .px(px(14.))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(8.))
        .bg(t.button)
        .text_color(t.button_ink)
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into())
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.opacity(0.86)))
        .when(!enabled, |d| d.opacity(0.35))
}

pub(crate) fn outline_button(id: impl Into<ElementId>, label: impl Into<SharedString>, t: &Theme, enabled: bool) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id)
        .h(px(32.))
        .px(px(13.))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .rounded(px(8.))
        .border_1()
        .border_color(t.line_strong)
        .bg(t.bg)
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into())
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .when(!enabled, |d| d.opacity(0.4))
}

/// A tool in a row, with its name: the timeline's Split and Delete.
fn tool(id: impl Into<ElementId>, name: &'static str, label: &'static str, t: &Theme, enabled: bool) -> Stateful<Div> {
    let hover = t.active;
    div()
        .id(id)
        .h(px(28.))
        .px(px(9.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(7.))
        .text_size(px(12.5))
        .text_color(t.ink_2)
        .child(glyph(name, 15., t.ink_2))
        .child(label)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .when(!enabled, |d| d.opacity(0.4))
}

/// What's changing, in loom's warm outline.
pub(crate) fn pill(t: &Theme) -> Div {
    div()
        .h(px(24.))
        .px(px(11.))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.))
        .rounded_full()
        .border_1()
        .border_color(t.warn)
        .text_color(t.warn)
        .text_size(px(12.))
        .whitespace_nowrap()
}

/// `label value` in monospace, the value in full ink.
fn chip(t: &Theme, label: &'static str, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .gap(px(6.))
        .whitespace_nowrap()
        .child(div().text_color(t.ink_3).child(label))
        .child(div().text_color(t.ink).child(value.into()))
}

fn info_row(t: &Theme, title: &'static str, chips: Vec<Div>) -> Div {
    div()
        .px(px(12.))
        .py(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(12.)).text_color(t.ink_2).child(title))
        .child(div().flex().flex_wrap().gap_x(px(14.)).gap_y(px(4.)).font_family(MONO).text_size(px(12.)).children(chips))
}

fn media_meta(m: &MediaItem) -> Option<String> {
    m.info.as_ref().map(|i| format!("{}×{} · {}p", i.width, i.height, (i.fps * 100.0).round() / 100.0))
}

/// A short clock: minutes and seconds, with hours when there are some.
pub(crate) fn short_time(t: f64) -> String {
    let whole = t.max(0.0).floor() as u64;
    let (h, m, s) = (whole / 3600, whole / 60 % 60, whole % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The ruler's labels, finer when the ticks are close.
fn ruler_label(t: f64, step: f64) -> String {
    let base = short_time(t);
    if step < 1.0 { format!("{base}.{}", ((t - t.floor()) * 10.0).round() as u64 % 10) } else { base }
}

/// A timecode without its hours while there are none.
fn compact_timecode(t: f64, fps: f64) -> String {
    let tc = timecode(t, fps);
    tc.strip_prefix("00:").map(str::to_string).unwrap_or(tc)
}

impl Galdr {
    fn render_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let item = |id: &'static str, name: &'static str, color: Rgba, on: bool| {
            let hover = t.hover;
            div()
                .id(id)
                .size(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .cursor_pointer()
                .when(on, |d| d.bg(t.active))
                .when(!on, move |d| d.hover(move |s| s.bg(hover)))
                .child(glyph(name, 20., color))
        };
        let page = self.page;
        div()
            .w(px(RAIL_W))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .border_r_1()
            .border_color(t.line)
            .child(
                div()
                    .h(px(BAR_H))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(icon_button("toggle-media", "panel-left", &t).on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Edit;
                        this.focus_view = false;
                        this.set_layout(|l| l.media = !l.media);
                        cx.notify();
                    }))),
            )
            .child(item("tool-edit", "scissors", t.tools[0], page == Page::Edit).on_click(cx.listener(
                |this, _, _, cx| {
                    this.page = Page::Edit;
                    cx.notify();
                },
            )))
            .children(TOOLS.iter().enumerate().map(|(i, (name, _))| {
                item(["tool-1", "tool-2", "tool-3", "tool-4"][i], name, t.tools[i + 1], page == Page::Tool(i)).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.page = Page::Tool(i);
                        this.focus_view = false;
                        cx.notify();
                    }),
                )
            }))
            .child(div().flex_1())
            .child(item("settings", "settings", t.ink_3, page == Page::Settings).on_click(cx.listener(
                |this, _, _, cx| {
                    this.page = Page::Settings;
                    this.focus_view = false;
                    cx.notify();
                },
            )))
            .child(div().h(px(12.)))
    }

    fn render_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let can_export = !self.project.clips.is_empty() && !self.exporting();
        let export = self.export_size().map(|(w, h, fps)| format!("{w}×{h} · {}p · H.264", (fps * 100.0).round() / 100.0));
        let progress = self.jobs.latest(Kind::Export).filter(|j| j.active()).map(|j| j.progress());
        let hover = t.hover;
        let control = |id: &'static str, name: &'static str, area: WindowControlArea, close: bool| {
            let (hover, ink) = if close { (t.bad, rgb(0xffffff)) } else { (t.hover, t.ink) };
            div()
                .id(id)
                .w(px(36.))
                .h(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .window_control_area(area)
                .group(id)
                .hover(move |s| s.bg(hover))
                .child(glyph(name, 15., t.ink_3).group_hover(id, move |s| s.text_color(ink)))
        };
        div()
            .h(px(BAR_H))
            .flex_none()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t.line)
            .child(
                div()
                    .id("title")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .pl(px(16.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(svg().path(icon("mark")).w(px(28.)).h(px(12.)).flex_none().text_color(t.ink))
                    .child(
                        div()
                            .relative()
                            .child(
                                div()
                                    .id("project-name")
                                    .h(px(30.))
                                    .px(px(8.))
                                    .ml(px(-8.))
                                    .flex()
                                    .items_center()
                                    .gap(px(7.))
                                    .rounded(px(7.))
                                    .cursor_pointer()
                                    .when(self.menu, |d| d.bg(t.active))
                                    .hover(move |s| s.bg(hover))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(14.))
                                            .child(SharedString::from(self.project_name())),
                                    )
                                    // Changed since it was saved.
                                    .when(self.dirty(), |d| d.child(div().size(px(6.)).rounded_full().bg(t.warn)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.menu = !this.menu;
                                        cx.notify();
                                    })),
                            )
                            .when(self.menu, |d| d.child(deferred(self.render_menu(cx)).with_priority(1))),
                    )
                    .children(self.notice.as_ref().map(|(text, _)| {
                        div().text_size(px(12.5)).text_color(t.ink_3).truncate().child(SharedString::from(text.clone()))
                    }))
                    .child(div().flex_1())
                    .children(progress.map(|f| {
                        pill(&t)
                            .child("Exporting")
                            .child(
                                div()
                                    .w(px(46.))
                                    .h(px(3.))
                                    .rounded_full()
                                    .bg(t.raised_2)
                                    .overflow_hidden()
                                    .child(div().h_full().w(relative(f)).bg(t.warn)),
                            )
                            .child(div().font_family(MONO).child(SharedString::from(format!("{:.0}%", f * 100.0))))
                    }))
                    .children(export.map(|s| {
                        div().text_size(px(12.5)).text_color(t.ink_3).font_family(MONO).child(SharedString::from(s))
                    }))
                    .child(button("export", "Export", &t, can_export).on_click(cx.listener(|this, _, _, cx| {
                        this.prompt_export(cx);
                        cx.notify();
                    })))
                    .child(icon_button("toggle-inspector", "panel-right", &t).on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Edit;
                        this.focus_view = false;
                        this.set_layout(|l| l.inspector = !l.inspector);
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .px(px(8.))
                    .child(control("win-min", "minus", WindowControlArea::Min, false))
                    .child(control(
                        "win-max",
                        if window.is_maximized() { "restore" } else { "square" },
                        WindowControlArea::Max,
                        false,
                    ))
                    .child(control("win-close", "x", WindowControlArea::Close, true)),
            )
    }

    /// The project menu, under its name.
    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let item = |id: &'static str, label: &'static str, keys: &'static str| {
            let hover = t.hover;
            div()
                .id(id)
                .h(px(32.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(24.))
                .rounded(px(7.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(div().flex_1().child(label))
                .child(div().font_family(MONO).text_size(px(11.5)).text_color(t.ink_3).child(keys))
        };
        div()
            .id("project-menu")
            .absolute()
            .top(px(36.))
            .left(px(-8.))
            .w(px(240.))
            .p(px(5.))
            .rounded(px(10.))
            .border_1()
            .border_color(t.line_strong)
            .bg(t.bg)
            .shadow_lg()
            .text_size(px(13.))
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.menu = false;
                cx.notify();
            }))
            .child(item("menu-new", "New", "Ctrl N").on_click(cx.listener(|this, _, window, cx| {
                this.menu = false;
                this.guard(crate::file::Then::New, window, cx);
                cx.notify();
            })))
            .child(item("menu-open", "Open…", "Ctrl O").on_click(cx.listener(|this, _, window, cx| {
                this.menu = false;
                this.guard(crate::file::Then::Open, window, cx);
                cx.notify();
            })))
            .child(div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(t.line))
            .child(item("menu-save", "Save", "Ctrl S").on_click(cx.listener(|this, _, _, cx| {
                this.menu = false;
                this.save(cx);
                cx.notify();
            })))
            .child(item("menu-save-as", "Save as…", "Ctrl Shift S").on_click(cx.listener(|this, _, _, cx| {
                this.menu = false;
                this.save_as(cx);
                cx.notify();
            })))
    }

    fn divider(&self, id: &'static str, divider: Divider, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let vertical = divider != Divider::Timeline;
        let active = matches!(self.drag, Some(Drag::Resize { divider: d, .. }) if d == divider);
        let group: SharedString = format!("{id}-group").into();
        let line = div()
            .rounded_full()
            .bg(t.line_strong)
            .opacity(if active { 1. } else { 0. })
            .group_hover(group.clone(), |s| s.opacity(1.))
            .when(vertical, |d| d.w(px(2.)).h(px(40.)))
            .when(!vertical, |d| d.h(px(2.)).w(px(40.)));
        div()
            .id(id)
            .group(group)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .when(vertical, |d| d.w(px(8.)).h_full().cursor_col_resize())
            .when(!vertical, |d| d.h(px(8.)).w_full().cursor_row_resize())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    this.start_resize(divider, e);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(line)
    }

    fn render_media(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let count = self.project.media.len();
        let body: AnyElement = if count == 0 {
            div()
                .mt(px(4.))
                .py(px(28.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(12.))
                .rounded(px(10.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .child(glyph("upload", 20., t.ink_3))
                .child(div().text_size(px(12.5)).text_color(t.ink_3).child("Drop video files here"))
                .child(
                    outline_button("import-empty", "Import", &t, true)
                        .on_click(cx.listener(|this, _, _, cx| this.prompt_import(cx))),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(self.project.media.iter().map(|m| self.render_media_row(m, cx).into_any_element()).collect::<Vec<_>>())
                .into_any_element()
        };
        div()
            .w(px(self.layout.media_w))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .pl(px(14.))
            .pt(px(12.))
            .child(
                div()
                    .h(px(32.))
                    .pl(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Media"))
                    .when(count > 0, |d| {
                        d.child(div().text_color(t.ink_3).text_size(px(12.)).child(SharedString::from(count.to_string())))
                    })
                    .child(div().flex_1())
                    .child(icon_button("import", "plus", &t).on_click(cx.listener(|this, _, _, cx| this.prompt_import(cx)))),
            )
            .child(div().id("media-list").flex_1().min_h_0().pt(px(6.)).overflow_y_scroll().child(body))
    }

    fn render_media_row(&self, m: &MediaItem, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let id = m.id;
        let group: SharedString = format!("media-{id}").into();
        let ready = m.info.is_some();
        let (meta, meta_color): (SharedString, Rgba) = match (media_meta(m), &m.error) {
            (Some(s), _) => (s.into(), t.ink_3),
            (None, Some(_)) => ("Can't read this file".into(), t.bad),
            (None, None) => ("Reading…".into(), t.ink_3),
        };
        let on = self.selected_media == Some(id);
        let hover = t.hover;
        div()
            .id(("media", id as usize))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(11.))
            .px(px(8.))
            .py(px(7.))
            .rounded(px(10.))
            .cursor_pointer()
            .when(on, |d| d.bg(t.active))
            .when(!on, move |d| d.hover(move |s| s.bg(hover)))
            .when(ready, |d| {
                let drag = MediaDrag {
                    id,
                    name: m.name.clone().into(),
                    thumb: self.thumbnails.get(&id).cloned(),
                    theme: t,
                };
                d.on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
            })
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                this.selected_media = Some(id);
                if e.click_count() >= 2 {
                    this.append(id);
                }
                cx.notify();
            }))
            .child(
                div()
                    .w(px(58.))
                    .h(px(33.))
                    .flex_none()
                    .rounded(px(5.))
                    .overflow_hidden()
                    .bg(t.raised_2)
                    .children(self.thumbnails.get(&id).cloned().map(|i| img(i).size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().child(SharedString::from(m.name.clone())))
                    .child(div().truncate().text_size(px(11.5)).text_color(meta_color).child(meta)),
            )
            .children(m.info.as_ref().map(|i| {
                div()
                    .flex_none()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .text_color(t.ink_3)
                    .group_hover(group.clone(), |s| s.opacity(0.))
                    .child(SharedString::from(short_time(i.duration)))
            }))
            .when(ready, |d| {
                d.child(
                    div()
                        .id(("append", id as usize))
                        .absolute()
                        .right(px(8.))
                        .size(px(26.))
                        .rounded(px(7.))
                        .bg(t.button)
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.append(id);
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .child(glyph("plus", 15., t.button_ink)),
                )
            })
    }

    fn render_stage(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let bounds = self.viewer_bounds.clone();
        let fps = self.fps();
        let playing = self.preview.mode == Mode::Playing;
        let place = self.project.place(self.preview.playhead);
        let info = place.and_then(|p| self.project.media(p.media)).and_then(media_meta);
        let has_clips = !self.project.clips.is_empty();
        let key = |id: &'static str, name: &'static str| {
            let hover = t.hover;
            div()
                .id(id)
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(glyph(name, 16., t.ink_2))
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .pt(px(14.))
            .when(self.focus_view, |d| d.px(px(14.)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(t.viewer)
                    .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().size_full())
                    .children(
                        self.preview
                            .image
                            .clone()
                            .filter(|_| has_clips)
                            // Pinned to the viewer, so the picture fits it whichever way is tighter.
                            .map(|i| img(i).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Contain)),
                    )
                    .when(!has_clips, |d| {
                        d.flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.))
                            .text_color(rgb(0x707079))
                            .child("Add media to the timeline")
                    }),
            )
            .child(
                div()
                    .h(px(58.))
                    .flex_none()
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .font_family(MONO)
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(SharedString::from(timecode(self.preview.playhead, fps))),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_3)
                                    .child(SharedString::from(format!("/ {}", timecode(self.project.duration(), fps)))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(key("to-start", "to-start").on_click(cx.listener(|this, _, _, cx| {
                                this.preview.seek(&this.project, 0.0, false);
                                cx.notify();
                            })))
                            .child(key("frame-back", "frame-back").on_click(cx.listener(|this, _, _, cx| {
                                this.step_frames(-1.0);
                                cx.notify();
                            })))
                            .child(
                                div()
                                    .id("play")
                                    .mx(px(4.))
                                    .size(px(38.))
                                    .rounded(px(10.))
                                    .bg(t.button)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.86))
                                    .child(glyph(if playing { "pause" } else { "play" }, 16., t.button_ink))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.preview.toggle(&this.project);
                                        cx.notify();
                                    })),
                            )
                            .child(key("frame-forward", "frame-forward").on_click(cx.listener(|this, _, _, cx| {
                                this.step_frames(1.0);
                                cx.notify();
                            })))
                            .child(key("to-end", "to-end").on_click(cx.listener(|this, _, _, cx| {
                                this.preview.seek(&this.project, this.project.duration(), false);
                                cx.notify();
                            }))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .text_color(t.ink_3)
                                    .children(info.map(SharedString::from)),
                            )
                            .child(icon_button("focus", "focus", &t).on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_focus_view();
                                cx.notify();
                            }))),
                    ),
            )
    }

    fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let fps = self.fps();
        let tc = |x: f64| timecode(x, fps);
        let selected = self.selected.and_then(|id| self.project.index_of(id));
        let card = div().p(px(14.)).rounded(px(14.)).border_1().border_color(t.line).bg(t.bg).flex().flex_col();
        let inner = || div().my(px(14.)).rounded(px(10.)).bg(t.side).flex().flex_col();
        let sep = || div().h(px(1.)).mx(px(12.)).bg(t.line);
        let card = if let Some(i) = selected {
            let c = &self.project.clips[i];
            let m = self.project.media(c.media);
            let info = m.and_then(|m| m.info.as_ref());
            let trimmed = c.source_in > 0.01 || info.is_some_and(|i| c.source_out < i.duration - 0.01);
            let start = self.project.clip_start(i);
            let under = self.preview.playhead > start + 0.05 && self.preview.playhead < start + c.length() - 0.05;
            let path = m.map(|m| m.path.clone());
            card.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(11.))
                    .child(
                        div()
                            .size(px(34.))
                            .flex_none()
                            .rounded(px(8.))
                            .overflow_hidden()
                            .bg(t.raised_2)
                            .children(
                                self.thumbnails.get(&c.media).cloned().map(|i| img(i).size_full().object_fit(ObjectFit::Cover)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.5))
                                    .child(SharedString::from(m.map(|m| m.name.clone()).unwrap_or_default())),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .font_family(MONO)
                                    .text_size(px(11.5))
                                    .text_color(t.ink_3)
                                    .child(SharedString::from(format!(
                                        "clip {} · {} → {}",
                                        i + 1,
                                        short_time(start),
                                        short_time(start + c.length())
                                    ))),
                            ),
                    )
                    .when(trimmed, |d| d.child(pill(&t).child("Trimmed"))),
            )
            .child(
                inner()
                    .child(info_row(
                        &t,
                        "Timing",
                        vec![chip(&t, "in", tc(c.source_in)), chip(&t, "out", tc(c.source_out)), chip(&t, "length", tc(c.length()))],
                    ))
                    .children(
                        info.map(|info| {
                            vec![
                                sep(),
                                info_row(
                                    &t,
                                    "Picture",
                                    vec![
                                        chip(&t, "size", format!("{}×{}", info.width, info.height)),
                                        chip(&t, "rate", format!("{}p", (info.fps * 100.0).round() / 100.0)),
                                        chip(&t, "codec", info.codec.to_uppercase()),
                                    ],
                                ),
                                sep(),
                                info_row(&t, "Sound", vec![chip(&t, "audio", if info.has_audio { "yes" } else { "none" })]),
                            ]
                        })
                        .into_iter()
                        .flatten(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(button("card-split", "Split here", &t, under).on_click(cx.listener(|this, _, _, cx| {
                        this.split();
                        cx.notify();
                    })))
                    .child(outline_button("card-delete", "Delete", &t, true).on_click(cx.listener(|this, _, _, cx| {
                        this.delete_selected();
                        cx.notify();
                    })))
                    .child(div().flex_1())
                    .children(path.map(|p| {
                        let hover = t.ink;
                        div()
                            .id("show-file")
                            .text_size(px(12.))
                            .text_color(t.ink_3)
                            .cursor_pointer()
                            .hover(move |s| s.text_color(hover))
                            .child("Show file ›")
                            .on_click(move |_, _, _| {
                                let _ =
                                    std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
                            })
                    })),
            )
        } else {
            let export = self.export_size();
            card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(13.5)).child("Timeline"))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(11.5))
                            .text_color(t.ink_3)
                            .child(SharedString::from(format!(
                                "{} clips · {}",
                                self.project.clips.len(),
                                short_time(self.project.duration())
                            ))),
                    ),
            )
            .child(
                inner()
                    .child(info_row(
                        &t,
                        "Timing",
                        vec![
                            chip(&t, "length", tc(self.project.duration())),
                            chip(&t, "rate", format!("{}p", (fps * 100.0).round() / 100.0)),
                        ],
                    ))
                    .children(
                        export
                            .map(|(w, h, fps)| {
                                vec![
                                    sep(),
                                    info_row(
                                        &t,
                                        "Export",
                                        vec![
                                            chip(&t, "size", format!("{w}×{h}")),
                                            chip(&t, "rate", format!("{}p", (fps * 100.0).round() / 100.0)),
                                            chip(&t, "format", "MP4 · H.264 · AAC"),
                                        ],
                                    ),
                                ]
                            })
                            .into_iter()
                            .flatten(),
                    ),
            )
        };

        // The newest few jobs of any kind, the export first.
        let jobs: Vec<AnyElement> = self
            .jobs
            .list
            .iter()
            .rev()
            .take(4)
            .map(|j| {
                let name: SharedString = j.output_name().into();
                let f = j.progress();
                let output = j.output.clone();
                let row = div().flex().items_center().gap(px(10.)).px(px(6.)).py(px(8.)).text_size(px(12.5));
                match j.state {
                    State::Finished => row
                        .child(div().flex_1().min_w_0().truncate().child(name))
                        .child(div().text_color(t.ok).child("Done"))
                        .child(
                            div()
                                .id(("reveal", j.id as usize))
                                .text_color(t.ink_3)
                                .cursor_pointer()
                                .child("Show ›")
                                .on_click(move |_, _, _| Galdr::reveal(&output)),
                        ),
                    State::Failed | State::Canceled => row
                        .flex_col()
                        .items_start()
                        .gap(px(2.))
                        .child(div().flex().gap(px(8.)).child(div().truncate().child(name)).child(
                            if j.state == State::Failed {
                                div().text_color(t.bad).child("Failed")
                            } else {
                                div().text_color(t.ink_3).child("Canceled")
                            },
                        ))
                        .children(j.error.clone().map(|e| {
                            div()
                                .text_size(px(11.5))
                                .text_color(t.ink_3)
                                .child(SharedString::from(e.lines().last().unwrap_or("").to_string()))
                        })),
                    State::Queued | State::Running => row
                        .child(div().flex_none().max_w(px(150.)).truncate().child(name))
                        .child(
                            div()
                                .flex_1()
                                .h(px(3.))
                                .rounded_full()
                                .bg(t.raised_2)
                                .overflow_hidden()
                                .child(div().h_full().w(relative(f)).bg(t.ink)),
                        )
                        .child(
                            div()
                                .w(px(36.))
                                .flex()
                                .justify_end()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .text_color(t.ink_3)
                                .child(SharedString::from(format!("{:.0}%", f * 100.0))),
                        ),
                }
                .into_any_element()
            })
            .collect();

        div()
            .id("inspector")
            .w(px(self.layout.inspector_w))
            .flex_none()
            .h_full()
            .pr(px(14.))
            .pt(px(14.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .child(card)
            .when(!jobs.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(18.))
                        .flex()
                        .flex_col()
                        .child(div().px(px(6.)).pb(px(4.)).font_weight(FontWeight::SEMIBOLD).child("Jobs"))
                        .children(jobs),
                )
            })
    }

    fn render_timeline(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        self.fit_zoom();
        let t = self.theme;
        let bounds = self.timeline_bounds.clone();
        let width = f32::from(bounds.get().size.width);
        let total = self.project.duration();
        let track_h = self.track_h();
        let fps = self.fps();

        // Ruler: labelled ticks about 90 px apart, with smaller ones between.
        let steps = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0];
        let step = steps.into_iter().find(|s| *s as f32 * self.zoom >= 90.0).unwrap_or(7200.0);
        let minor = step / 5.0;
        let first = ((self.scroll - LANE_PAD) / self.zoom / minor as f32).floor().max(0.0) as i64;
        let last = ((self.scroll + width) / self.zoom / minor as f32).ceil() as i64;
        let ticks = (first..=last).map(|i| {
            let tt = i as f64 * minor;
            let major = i % 5 == 0;
            div()
                .absolute()
                .left(px(self.x_of(tt)))
                .top_0()
                .h(px(RULER_H))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .bottom(px(3.))
                        .w(px(1.))
                        .h(px(if major { 7. } else { 3. }))
                        .bg(t.line_strong),
                )
                .when(major, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(4.))
                            .top(px(6.))
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(t.ink_3)
                            .whitespace_nowrap()
                            .child(SharedString::from(ruler_label(tt, step))),
                    )
                })
        });

        let moving = match self.drag {
            Some(Drag::Move { clip, x, moved: true, .. }) => Some((clip, self.drop_slot(clip, x).1)),
            _ => None,
        };
        let clips: Vec<AnyElement> = (0..self.project.clips.len())
            .flat_map(|i| {
                [self.render_clip(i, width, moving.map(|m| m.0)), self.render_clip_audio(i, width, moving.map(|m| m.0))]
            })
            .collect();
        let playhead_x = self.x_of(self.preview.playhead);

        let lane = div()
            .id("lane")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().size_full())
            // Files dragged from the media: a marker where they'd go, and in on the drop.
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<MediaDrag>, _, cx| {
                let inside = e.bounds.contains(&e.event.position);
                let x = inside.then(|| f32::from(e.event.position.x) - f32::from(e.bounds.origin.x));
                if x != this.media_drop {
                    this.media_drop = x;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, d: &MediaDrag, _, cx| {
                let x = this.media_drop.take().unwrap_or(f32::MAX);
                this.drop_media(d.id, x);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    this.timeline_mouse_down(e);
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| {
                let d = e.delta.pixel_delta(window.line_height());
                if e.modifiers.control {
                    this.zoom_by(if f32::from(d.y) > 0.0 { 1.15 } else { 1.0 / 1.15 });
                } else {
                    let delta = if f32::from(d.x).abs() > f32::from(d.y).abs() { d.x } else { d.y };
                    this.fit = false;
                    this.scroll = (this.scroll - f32::from(delta)).max(0.0);
                }
                cx.notify();
            }))
            .children(ticks)
            .children(clips)
            .children(moving.map(|(_, gap_x)| gap_x).or(self.media_drop.map(|x| self.gap_at(x).1)).map(|gap_x| {
                div()
                    .absolute()
                    .left(px(gap_x - 1.5))
                    .top(px(TRACK_TOP - 5.0))
                    .w(px(3.))
                    .h(px(self.audio_top() + self.audio_h() - TRACK_TOP + 10.0))
                    .rounded_full()
                    .bg(t.ink)
            }))
            .when(total > 0.0, |d| {
                d.child(div().absolute().left(px(playhead_x - 0.75)).top(px(RULER_H - 4.0)).w(px(1.5)).bottom_0().bg(t.ink))
                    .child(
                        div().absolute().left(px(playhead_x - 40.0)).top(px(3.)).w(px(80.)).flex().justify_center().child(
                            div()
                                .px(px(6.))
                                .py(px(3.))
                                .rounded(px(5.))
                                .bg(t.button)
                                .text_color(t.button_ink)
                                .font_family(MONO)
                                .text_size(px(11.))
                                .child(SharedString::from(compact_timecode(self.preview.playhead, fps))),
                        ),
                    )
            });

        div()
            .h(px(self.layout.timeline_h))
            .flex_none()
            .mx(px(14.))
            .mb(px(14.))
            .rounded(px(14.))
            .border_1()
            .border_color(t.line)
            .bg(t.side)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(TOOLS_H))
                    .flex_none()
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(tool("split", "split", "Split", &t, total > 0.0).on_click(cx.listener(|this, _, _, cx| {
                        this.split();
                        cx.notify();
                    })))
                    .child(tool("delete", "trash", "Delete", &t, self.selected.is_some()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.delete_selected();
                            cx.notify();
                        },
                    )))
                    .child(div().w(px(1.)).h(px(16.)).mx(px(6.)).bg(t.line_strong))
                    .child(
                        icon_button("undo", "undo", &t)
                            .when(!self.project.can_undo(), |d| d.opacity(0.35))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.undo();
                                cx.notify();
                            })),
                    )
                    .child(
                        icon_button("redo", "redo", &t)
                            .when(!self.project.can_redo(), |d| d.opacity(0.35))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.redo();
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(icon_button("zoom-out", "minus", &t).on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_by(0.8);
                        cx.notify();
                    })))
                    .child(tool("fit", "focus", "Fit", &t, true).on_click(cx.listener(|this, _, _, cx| {
                        this.fit = true;
                        cx.notify();
                    })))
                    .child(icon_button("zoom-in", "plus", &t).on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_by(1.25);
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(HEADS_W))
                            .flex_none()
                            .relative()
                            .text_size(px(11.))
                            .text_color(t.ink_3)
                            .child(
                                div()
                                    .absolute()
                                    .top(px(TRACK_TOP))
                                    .h(px(track_h))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("V1"),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(self.audio_top()))
                                    .h(px(self.audio_h()))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("A1"),
                            ),
                    )
                    .child(lane),
            )
    }

    fn render_clip(&self, i: usize, lane_w: f32, moving: Option<crate::project::ClipId>) -> AnyElement {
        let t = self.theme;
        let c = &self.project.clips[i];
        let x0 = self.x_of(self.project.clip_start(i));
        let w = (c.length() as f32 * self.zoom - 3.0).max(3.0);
        let track_h = self.track_h();
        let selected = self.selected == Some(c.id);
        let media = self.project.media(c.media);
        let name = media.map(|m| m.name.clone()).unwrap_or_default();
        let aspect = media.and_then(|m| m.info.as_ref()).map_or(16.0 / 9.0, |i| i.width as f32 / i.height.max(1) as f32);
        let group: SharedString = format!("clip-{}", c.id).into();

        // Frames along the whole clip, with the name and length on a label over the first.
        let tile_w = (track_h * aspect).max(24.0);
        let show_text = w >= 90.0;
        let strip = self.strips.get(&c.media).filter(|s| !s.is_empty());
        let frame_at = |x: f32| -> Option<Arc<gpui::RenderImage>> {
            let strip = strip?;
            let ts = c.source_in + (x / self.zoom) as f64;
            let j = strip.partition_point(|(st, _)| *st < ts);
            let pick = if j == 0 {
                0
            } else if j >= strip.len() || (ts - strip[j - 1].0) < (strip[j].0 - ts) {
                j - 1
            } else {
                j
            };
            Some(strip[pick.min(strip.len() - 1)].1.clone())
        };
        let mut tiles = Vec::new();
        if strip.is_none() {
            if let Some(image) = self.thumbnails.get(&c.media).cloned() {
                tiles.push(img(image).absolute().left_0().top_0().w(px(tile_w)).h_full().object_fit(ObjectFit::Cover));
            }
        } else {
            let k0 = ((-x0) / tile_w).floor().max(0.0) as usize;
            let k1 = (((lane_w - x0) / tile_w).ceil().max(0.0) as usize).min((w / tile_w).ceil() as usize);
            for k in k0..k1 {
                let x = k as f32 * tile_w;
                if let Some(image) = frame_at(x + tile_w / 2.0) {
                    tiles.push(
                        img(image)
                            .absolute()
                            .left(px(if k == 0 { 0.0 } else { x + 1.0 }))
                            .top_0()
                            .w(px(if k == 0 { tile_w } else { tile_w - 1.0 }))
                            .h_full()
                            .object_fit(ObjectFit::Cover),
                    );
                }
            }
        }
        let handle = |left: bool| {
            div()
                .absolute()
                .top(px(6.))
                .bottom(px(6.))
                .when(left, |d| d.left(px(3.)))
                .when(!left, |d| d.right(px(3.)))
                .w(px(4.))
                .rounded_full()
                .bg(t.ink)
                .cursor_ew_resize()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(0.9))
        };
        div()
            .absolute()
            .left(px(x0))
            .top(px(TRACK_TOP))
            .w(px(w))
            .h(px(track_h))
            .group(group.clone())
            .rounded(px(8.))
            .overflow_hidden()
            .bg(t.bg)
            .when(moving == Some(c.id), |d| d.opacity(0.5))
            .children(tiles)
            .when(show_text, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(px(6.))
                        .top(px(6.))
                        .max_w(px((w - 12.0).max(0.0)))
                        .h(px(22.))
                        .px(px(7.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .rounded(px(6.))
                        .bg(t.bg)
                        .text_size(px(11.5))
                        .child(div().min_w_0().truncate().child(SharedString::from(name)))
                        .child(
                            div()
                                .flex_none()
                                .font_family(MONO)
                                .text_size(px(11.))
                                .text_color(t.ink_3)
                                .child(SharedString::from(short_time(c.length()))),
                        ),
                )
            })
            .when(w > 40.0, |d| d.child(handle(true)).child(handle(false)))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(8.))
                    .when(selected, |d| d.border_2().border_color(t.ink))
                    .when(!selected, |d| d.border_1().border_color(t.line_strong)),
            )
            .into_any_element()
    }

    /// A clip's sound in the audio lane, under its picture: the waveform, loudest at the middle.
    fn render_clip_audio(&self, i: usize, lane_w: f32, moving: Option<crate::project::ClipId>) -> AnyElement {
        let t = self.theme;
        let c = &self.project.clips[i];
        let x0 = self.x_of(self.project.clip_start(i));
        let w = (c.length() as f32 * self.zoom - 3.0).max(3.0);
        let selected = self.selected == Some(c.id);
        let has_audio = self.project.media(c.media).and_then(|m| m.info.as_ref()).is_some_and(|i| i.has_audio);
        let peaks = self.waveforms.get(&c.media).cloned();
        let (source_in, zoom) = (c.source_in, self.zoom);
        // Only the part on screen is drawn.
        let (vis0, vis1) = ((-x0).max(0.0), (lane_w - x0).min(w));
        let color = t.ink_3;
        let waveform = canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                let Some(peaks) = peaks else { return };
                let h = f32::from(b.size.height) - 8.0;
                let mid = f32::from(b.origin.y) + f32::from(b.size.height) / 2.0;
                let per_px = PEAKS_PER_SECOND as f64 / zoom as f64;
                let mut x = vis0.floor();
                while x < vis1 {
                    let a = ((source_in + x as f64 / zoom as f64) * PEAKS_PER_SECOND as f64) as usize;
                    let z = (a + (per_px * 2.0).ceil().max(1.0) as usize).min(peaks.len());
                    let peak = peaks.get(a..z).map_or(0.0, |p| p.iter().copied().fold(0.0f32, f32::max));
                    // On a decibel scale, 48 dB deep, as editors show it.
                    let level = if peak > 0.0 { (1.0 + 20.0 * peak.log10() / 48.0).clamp(0.0, 1.0) } else { 0.0 };
                    let bar = (level * h).max(1.0);
                    window.paint_quad(fill(
                        Bounds::new(point(b.origin.x + px(x), px(mid - bar / 2.0)), size(px(1.2), px(bar))),
                        color,
                    ));
                    x += 2.0;
                }
            },
        )
        .absolute()
        .size_full();
        div()
            .absolute()
            .left(px(x0))
            .top(px(self.audio_top()))
            .w(px(w))
            .h(px(self.audio_h()))
            .rounded(px(8.))
            .overflow_hidden()
            .bg(if has_audio { t.bg } else { t.raised })
            .when(moving == Some(c.id), |d| d.opacity(0.5))
            .when(has_audio, |d| d.child(waveform))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(8.))
                    .when(selected, |d| d.border_2().border_color(t.ink))
                    .when(!selected, |d| d.border_1().border_color(t.line_strong)),
            )
            .into_any_element()
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let segment = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_size(px(12.5))
                .cursor_pointer()
                .when(on, |d| d.bg(t.bg).border_1().border_color(t.line_strong).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.text_color(t.ink_3))
                .child(label)
        };
        let row = |title: &'static str, control: AnyElement| {
            div()
                .min_h(px(54.))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(t.line)
                .child(div().flex_1().child(title))
                .child(control)
        };
        let light = t.mode == theme::Mode::Light;
        div().id("settings").flex_1().min_h_0().overflow_y_scroll().flex().justify_center().pt(px(48.)).pb(px(48.)).child(
            div()
                .w(px(600.))
                .flex()
                .flex_col()
                .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).pb(px(16.)).child("Settings"))
                .child(row(
                    "Theme",
                    div()
                        .flex()
                        .p(px(2.))
                        .gap(px(2.))
                        .rounded(px(8.))
                        .bg(t.raised)
                        .child(segment("theme-light", "Light", light).on_click(cx.listener(|this, _, _, cx| {
                            this.set_layout(|l| l.theme = theme::Mode::Light);
                            cx.notify();
                        })))
                        .child(segment("theme-dark", "Dark", !light).on_click(cx.listener(|this, _, _, cx| {
                            this.set_layout(|l| l.theme = theme::Mode::Dark);
                            cx.notify();
                        })))
                        .into_any_element(),
                ))
                .child(row(
                    "Layout",
                    outline_button("reset-layout", "Reset", &t, true)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let theme = this.layout.theme;
                            this.set_layout(|l| *l = crate::layout::Layout { theme, ..Default::default() });
                            cx.notify();
                        }))
                        .into_any_element(),
                ))
                .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).pt(px(36.)).pb(px(8.)).child("Keys"))
                .children(KEYS.iter().map(|(keys, what)| {
                    div()
                        .min_h(px(36.))
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(t.line)
                        .child(div().flex_1().text_color(t.ink_2).child(*what))
                        .child(
                            div().flex().gap(px(4.)).children(keys.split(" / ").map(|k| {
                                div()
                                    .px(px(7.))
                                    .py(px(2.))
                                    .rounded(px(5.))
                                    .bg(t.raised)
                                    .font_family(MONO)
                                    .text_size(px(11.5))
                                    .child(k)
                            })),
                        )
                })),
        )
    }

    fn render_edit(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus_view;
        let show_media = self.layout.media && !focus;
        let show_inspector = self.layout.inspector && !focus;
        let work = div()
            .flex_1()
            .min_h_0()
            .flex()
            .when(show_media, |d| d.child(self.render_media(cx)).child(self.divider("divider-media", Divider::Media, cx)))
            .when(!show_media && !focus, |d| d.pl(px(14.)))
            .child(self.render_stage(cx))
            .when(show_inspector, |d| {
                d.child(self.divider("divider-inspector", Divider::Inspector, cx)).child(self.render_inspector(cx))
            })
            .when(!show_inspector && !focus, |d| d.pr(px(14.)));
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(work)
            .when(!focus, |d| d.child(self.divider("divider-timeline", Divider::Timeline, cx)).child(self.render_timeline(cx)))
            .when(focus, |d| d.pb(px(6.)))
    }
}

impl Render for Galdr {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        auto::step(self, cx);
        snapshot::step(self, window, cx);
        let vb = self.viewer_bounds.get();
        let scale = window.scale_factor();
        self.preview
            .viewer_size((f32::from(vb.size.width) * scale).round() as u32, (f32::from(vb.size.height) * scale).round() as u32);
        let busy = self.preview.tick(&self.project, window, cx);
        if busy || self.auto.is_some() || self.snapshot.is_some() || self.drag.is_some() {
            window.request_animation_frame();
        }
        let t = self.theme;
        let page: AnyElement = match self.page {
            Page::Edit => self.render_edit(cx).into_any_element(),
            Page::Tool(i) => self.render_tool_page(i, cx).into_any_element(),
            Page::Settings => self.render_settings(cx).into_any_element(),
        };
        div()
            .id("root")
            .track_focus(self.focus_handle())
            .size_full()
            .flex()
            .bg(t.bg)
            .text_color(t.ink)
            .font_family(FONT)
            .text_size(px(13.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if this.on_key(e, window, cx) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.mouse_move(e);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    this.mouse_up(e);
                    cx.notify();
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.dropped(paths.paths().to_vec(), cx);
            }))
            .child(self.render_rail(cx))
            .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(self.render_bar(window, cx)).child(page))
    }
}

/// The keyboard, for the settings page: the keys, and what they do.
const KEYS: [(&str, &str); 13] = [
    ("Space", "Play or pause"),
    ("← / →", "One frame back or forward"),
    ("Home / End", "To the start or the end"),
    ("S", "Split at the playhead"),
    ("Delete", "Delete the selected clip"),
    ("Ctrl Z / Ctrl Shift Z", "Undo, redo"),
    ("= / - / 0", "Zoom in, out, to fit"),
    ("F", "Only the viewer"),
    ("F11", "Full screen"),
    ("Esc", "Back from either"),
    ("Ctrl [ / Ctrl ]", "Show or hide the media, the clip"),
    ("Ctrl S / Ctrl Shift S", "Save, save as"),
    ("Ctrl O / Ctrl N", "Open, new"),
];
