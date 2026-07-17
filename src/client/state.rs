#[derive(Debug)]
pub enum GameState {
    MainMenu,
    AltManager,
    Multiplayer,
    DirectConnect,
    ServerEditor {
        edit_index: Option<usize>,
    },
    Connecting,
    LoadingWorld,
    Playing,
    Paused,
    Disconnected {
        reason: String,
    },
    Options {
        previous: Box<GameState>,
    },
    VideoSettings {
        previous: Box<GameState>,
    },
    Controls {
        previous: Box<GameState>,
    },
    SkinCustomization {
        previous: Box<GameState>,
    },
    Language {
        previous: Box<GameState>,
    },
    AudioSettings {
        previous: Box<GameState>,
    },
    ChatSettings {
        previous: Box<GameState>,
    },
    ResourcePacks {
        previous: Box<GameState>,
    },
    ShaderPacks {
        previous: Box<GameState>,
    },
    Modding {
        previous: Box<GameState>,
    },
    ModConfig {
        previous: Box<GameState>,
        mod_id: String,
    },
}

impl GameState {
    pub fn menu_id(&self) -> u32 {
        match self {
            GameState::MainMenu => 0,
            GameState::AltManager => 16,
            GameState::Playing => 1,
            GameState::Paused => 2,
            GameState::Disconnected { .. } => 14,
            GameState::Options { .. } => 3,
            GameState::Multiplayer => 4,
            GameState::DirectConnect => 5,
            GameState::VideoSettings { .. } => 6,
            GameState::Controls { .. } => 7,
            GameState::Language { .. } => 8,
            GameState::AudioSettings { .. } => 9,
            GameState::ChatSettings { .. } => 20,
            GameState::SkinCustomization { .. } => 10,
            GameState::Connecting => 11,
            GameState::LoadingWorld => 12,
            GameState::ResourcePacks { .. } => 13,
            GameState::ShaderPacks { .. } => 19,
            GameState::ServerEditor { .. } => 15,
            GameState::Modding { .. } => 17,
            GameState::ModConfig { .. } => 18,
        }
    }

    pub fn is_menu(&self) -> bool {
        matches!(
            self,
            GameState::MainMenu
                | GameState::Multiplayer
                | GameState::AltManager
                | GameState::DirectConnect
                | GameState::ServerEditor { .. }
                | GameState::Connecting
                | GameState::LoadingWorld
                | GameState::Paused
                | GameState::Disconnected { .. }
                | GameState::Options { .. }
                | GameState::VideoSettings { .. }
                | GameState::Controls { .. }
                | GameState::SkinCustomization { .. }
                | GameState::Language { .. }
                | GameState::AudioSettings { .. }
                | GameState::ChatSettings { .. }
                | GameState::ResourcePacks { .. }
                | GameState::ShaderPacks { .. }
                | GameState::Modding { .. }
                | GameState::ModConfig { .. }
        )
    }

    pub fn has_world_background(&self) -> bool {
        match self {
            GameState::Playing | GameState::Paused => true,
            GameState::Options { previous }
            | GameState::VideoSettings { previous }
            | GameState::Controls { previous }
            | GameState::SkinCustomization { previous }
            | GameState::Language { previous }
            | GameState::AudioSettings { previous }
            | GameState::ChatSettings { previous }
            | GameState::ResourcePacks { previous }
            | GameState::ShaderPacks { previous }
            | GameState::Modding { previous }
            | GameState::ModConfig { previous, .. } => previous.has_world_background(),
            _ => false,
        }
    }
}
