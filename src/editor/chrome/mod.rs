//! Editor chrome: the tool rail, breadcrumbs, hamburger menu, bottom bar, the
//! per-object context menu, and the screenshot region selector.

mod bottom_bar;
mod breadcrumbs;
mod context_menu;
mod main_menu;
mod shortcuts;
mod shot_overlay;
mod toolbar;

pub use bottom_bar::BottomBar;
pub use breadcrumbs::Breadcrumbs;
pub use context_menu::ObjectMenu;
pub use main_menu::MainMenu;
pub use shot_overlay::ShotOverlay;
pub use toolbar::Toolbar;
