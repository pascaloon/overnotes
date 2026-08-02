//! The canvas element registry.
//!
//! Every kind of object on the canvas is described by one [`CanvasElement`]
//! implementation living in its own module. Adding an element means adding an
//! [`ObjectKind`] variant, writing that module, and listing it in `ELEMENTS`.

use dioxus::prelude::*;

use super::{drawing, image, note, subgraph};
use crate::editor::EditorState;
use crate::store::ObjectKind;

/// The active canvas tool. `Element(id)` refers to a [`ToolSpec::id`].
#[derive(Clone, Copy, PartialEq, Default)]
pub enum Tool {
    #[default]
    Select,
    Element(&'static str),
}

impl Tool {
    pub fn is(self, id: &str) -> bool {
        matches!(self, Tool::Element(active) if active == id)
    }
}

/// Everything an element needs in order to render one object.
pub struct ObjectCtx {
    pub id: u64,
    pub kind: ObjectKind,
    pub state: EditorState,
    /// This object's inline editor is open.
    pub editing: bool,
}

/// A toolbar tool that creates objects of one element type.
#[derive(Clone, Copy)]
pub struct ToolSpec {
    pub id: &'static str,
    pub tooltip: &'static str,
    /// Extra viewport class while the tool is active, used to set the cursor.
    pub cursor_class: &'static str,
    pub icon: fn() -> Element,
    /// Panel rendered under the toolbar while the tool is active.
    pub options: Option<fn(EditorState) -> Element>,
    /// Primary press on empty canvas, in world coordinates.
    pub on_press: fn(&mut EditorState, (f64, f64)),
}

/// A one-shot toolbar button that has no canvas mode, e.g. "paste image".
#[derive(Clone, Copy)]
pub struct ActionSpec {
    pub tooltip: &'static str,
    pub icon: fn() -> Element,
    pub run: fn(&mut EditorState),
}

pub trait CanvasElement: Sync {
    /// Whether this element owns the given object kind.
    fn matches(&self, kind: &ObjectKind) -> bool;

    /// The object's contents, rendered inside the shared positioned wrapper.
    fn body(&self, cx: &ObjectCtx) -> Element;

    /// Extra controls for the floating toolbar shown above a lone selection.
    fn toolbar(&self, _cx: &ObjectCtx) -> Option<Element> {
        None
    }

    /// Double-click on the object body.
    fn on_activate(&self, _state: &mut EditorState, _id: u64) {}

    /// Whether Shift-resize should preserve the object's current ratio.
    fn locks_aspect_ratio(&self) -> bool {
        false
    }

    fn tool(&self) -> Option<ToolSpec> {
        None
    }

    fn action(&self) -> Option<ActionSpec> {
        None
    }

    /// Stylesheet loaded alongside the editor. Order follows `ELEMENTS`.
    fn style(&self) -> Option<Asset> {
        None
    }
}

/// Registration list. Order drives toolbar order and stylesheet order.
static ELEMENTS: &[&dyn CanvasElement] = &[
    &note::Note,
    &subgraph::Subgraph,
    &drawing::Drawing,
    &image::Image,
];

/// Used for object kinds no element claims, so an unrecognised kind degrades
/// to an empty box instead of panicking mid-render.
struct Unsupported;

impl CanvasElement for Unsupported {
    fn matches(&self, _kind: &ObjectKind) -> bool {
        true
    }

    fn body(&self, _cx: &ObjectCtx) -> Element {
        rsx! {}
    }
}

static UNSUPPORTED: Unsupported = Unsupported;

pub fn element_for(kind: &ObjectKind) -> &'static dyn CanvasElement {
    ELEMENTS
        .iter()
        .copied()
        .find(|element| element.matches(kind))
        .unwrap_or(&UNSUPPORTED)
}

pub fn tools() -> impl Iterator<Item = ToolSpec> {
    ELEMENTS.iter().filter_map(|element| element.tool())
}

pub fn actions() -> impl Iterator<Item = ActionSpec> {
    ELEMENTS.iter().filter_map(|element| element.action())
}

pub fn styles() -> impl Iterator<Item = Asset> {
    ELEMENTS.iter().filter_map(|element| element.style())
}

pub fn tool_spec(tool: Tool) -> Option<ToolSpec> {
    match tool {
        Tool::Select => None,
        Tool::Element(id) => tools().find(|spec| spec.id == id),
    }
}

/// Viewport class for the active tool, which drives the cursor.
pub fn cursor_class(tool: Tool) -> &'static str {
    tool_spec(tool)
        .map(|spec| spec.cursor_class)
        .unwrap_or("tool-select")
}
