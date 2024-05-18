use bevy::log::info;
use bevy::prelude::World;
use bevy_editor_pls_core::Editor;
use crate::cameras::{CameraWindow, EditorCamKind, set_active_editor_camera_marker};

impl CameraWindow {
	/// Sets current active editor camera to the specified `EditorCamKind`
	pub fn set_active_camera(&self,
							 camera: EditorCamKind,
							 world: &mut World,
							 editor: &mut Editor,
	)
	{
		let Some(state) = editor.window_state_mut::<CameraWindow>() else {
			info!("Unable to fetch CameraWindowState. Check if CameraWindow is available");
			return;
		};

		if state.editor_cam != camera {
			set_active_editor_camera_marker(world, camera);
		}

		state.editor_cam = camera;
	}
}