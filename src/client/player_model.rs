#[derive(Clone, Copy, Debug)]
pub enum PlayerModelPart {
    Head,
    Body,
    RightArm,
    LeftArm,
    RightLeg,
    LeftLeg,
    Hat,
    Jacket,
    RightSleeve,
    LeftSleeve,
    RightPants,
    LeftPants,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelCuboid {
    pub part: PlayerModelPart,
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: [u32; 2],
    pub mirror: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerPose {
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub limb_swing: f32,
    pub swing_progress: f32,
    pub sneaking: bool,
}

impl Default for PlayerPose {
    fn default() -> Self {
        Self {
            body_yaw: 0.0,
            head_yaw: 0.0,
            pitch: 0.0,
            limb_swing: 0.0,
            swing_progress: 0.0,
            sneaking: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerModel {
    pub slim_arms: bool,
    pub parts: Vec<ModelCuboid>,
}

impl PlayerModel {
    pub fn steve() -> Self {
        Self::new(false)
    }

    pub fn alex() -> Self {
        Self::new(true)
    }

    fn new(slim_arms: bool) -> Self {
        let arm_w = if slim_arms { 3.0 } else { 4.0 };
        let parts = vec![
            ModelCuboid {
                part: PlayerModelPart::Head,
                origin: [-4.0, 24.0, -4.0],
                size: [8.0, 8.0, 8.0],
                uv: [0, 0],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::Hat,
                origin: [-4.5, 23.5, -4.5],
                size: [9.0, 9.0, 9.0],
                uv: [32, 0],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::Body,
                origin: [-4.0, 12.0, -2.0],
                size: [8.0, 12.0, 4.0],
                uv: [16, 16],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::RightArm,
                origin: [-4.0 - arm_w, 12.0, -2.0],
                size: [arm_w, 12.0, 4.0],
                uv: [40, 16],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::LeftArm,
                origin: [4.0, 12.0, -2.0],
                size: [arm_w, 12.0, 4.0],
                uv: [32, 48],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::RightLeg,
                origin: [-4.0, 0.0, -2.0],
                size: [4.0, 12.0, 4.0],
                uv: [0, 16],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::LeftLeg,
                origin: [0.0, 0.0, -2.0],
                size: [4.0, 12.0, 4.0],
                uv: [16, 48],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::Jacket,
                origin: [-4.25, 11.75, -2.25],
                size: [8.5, 12.5, 4.5],
                uv: [16, 32],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::RightSleeve,
                origin: [-4.25 - arm_w, 11.75, -2.25],
                size: [arm_w + 0.5, 12.5, 4.5],
                uv: [40, 32],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::LeftSleeve,
                origin: [3.75, 11.75, -2.25],
                size: [arm_w + 0.5, 12.5, 4.5],
                uv: [48, 48],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::RightPants,
                origin: [-4.25, -0.25, -2.25],
                size: [4.5, 12.5, 4.5],
                uv: [0, 32],
                mirror: false,
            },
            ModelCuboid {
                part: PlayerModelPart::LeftPants,
                origin: [-0.25, -0.25, -2.25],
                size: [4.5, 12.5, 4.5],
                uv: [0, 48],
                mirror: false,
            },
        ];
        Self { slim_arms, parts }
    }
}
