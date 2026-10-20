use crate::{
    lighting::{SceneLightSourceItem, SceneLightSuppressionKey},
    scene::{move_to_world_position, SceneObjectPosition},
    math::Vec3,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneObjectKind {
    Building,
    Unit,
    Object,
}

/// Rich editor scene object representation matching C# `MapSceneObject`.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneObject {
    pub name: String,
    pub world: Vec3,
    pub local: Vec3,
    pub team: i32,
    pub source_file: String,
    pub object_index: i32,
    pub angle: Option<f32>,
    pub has_native_saved_position: bool,
}

impl SceneObject {
    pub fn new(
        name: impl Into<String>,
        world: Vec3,
        local: Vec3,
        team: i32,
        source_file: impl Into<String>,
        object_index: i32,
        angle: Option<f32>,
    ) -> Self {
        Self {
            name: name.into(),
            world,
            local,
            team,
            source_file: source_file.into(),
            object_index,
            angle,
            has_native_saved_position: false,
        }
    }

    pub fn kind(&self) -> SceneObjectKind {
        if self.name.to_ascii_lowercase().starts_with("bau") {
            SceneObjectKind::Building
        } else if self.name.to_ascii_lowercase().starts_with("fig") {
            SceneObjectKind::Unit
        } else {
            SceneObjectKind::Object
        }
    }

    /// Whether an object without an active sprite gets a visible marker on the 3D map.
    /// Invisible DATA/objects.dat entries are excluded unless they are script marks.
    pub fn marker_visible(&self) -> bool {
        if self.source_file.eq_ignore_ascii_case("DATA/objects.dat") {
            self.name.to_ascii_lowercase().starts_with("skriptmark")
        } else {
            true
        }
    }

    pub fn suppression_key(&self) -> Option<SceneLightSuppressionKey> {
        SceneLightSuppressionKey::try_for(&self.source_file, self.object_index)
    }

    pub fn position(&self) -> SceneObjectPosition {
        SceneObjectPosition {
            world: self.world,
            local_x: self.local.x,
            local_z: self.local.z,
            has_native_saved_position: self.has_native_saved_position,
        }
    }

    pub fn as_light_source_item(&self) -> SceneLightSourceItem {
        SceneLightSourceItem::new(
            &self.name,
            self.world.x,
            self.world.y,
            self.world.z,
            &self.source_file,
            self.object_index,
        )
    }

    /// Safely moves the scene object to new world X and Z coordinates with boundary clamping.
    pub fn move_to(&self, x: f32, z: f32) -> Result<Self, &'static str> {
        if !x.is_finite() || !z.is_finite() {
            return Err("scene object coordinates must be finite");
        }
        let pos = move_to_world_position(self.position(), x, z)?;
        Ok(Self {
            world: pos.world,
            local: Vec3::new(pos.local_x, self.local.y, pos.local_z),
            ..self.clone()
        })
    }
}
