use bevy::prelude::World;
use crate::cameras::{CameraWindowState, EditorCamKind, set_active_editor_camera_marker};

impl CameraWindowState {
	/// Sets current active editor camera to the specified `EditorCamKind`
	pub fn set_active_camera(&mut self,
							 camera: EditorCamKind,
							 world: &mut World,
	)
	{
		if self.editor_cam != camera {
			set_active_editor_camera_marker(world, camera);
		}

		self.editor_cam = camera;
	}
}