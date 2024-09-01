use egui_dock::egui::Pos2;

/// Current state of the pointer used inside editor window
#[derive(Default)]
pub struct EditorPointerState {
	pub press_active: bool,
	pub press_start_in_viewport: bool,

	/// Position of the cursor inside the viewport / game view
	pub viewport_pointer_pos: Option<Pos2>,
}

impl EditorPointerState {
	/// Returns true if pointer is currently inside the viewport
	/// (excluding floating windows interaction)
	pub fn is_pointer_in_viewport(&self) -> bool {
		self.viewport_pointer_pos.is_some()
	}
}