/// Game state category for state machine queries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StateCategory {
    /// Main menu and its sub-screens (multiplayer, alt manager, direct connect, server editor).
    Menu,
    /// Actively playing with a world loaded.
    Playing,
    /// Paused while a world is loaded.
    Paused,
    /// Connecting to a server.
    Connecting,
    /// Loading the world after connection.
    LoadingWorld,
    /// Disconnected from server.
    Disconnected,
    /// A settings subscreen that stacks on a previous state.
    Subscreen,
}

/// State machine trait for game state transitions.
///
/// Implementors provide category-based queries, parent navigation for stacked
/// subscreens, and a safe transition method that validates the target state.
pub trait GameStateMachine {
    /// The broad category this state belongs to.
    fn category(&self) -> StateCategory;

    /// Whether this state is actively in-game (world loaded, player can interact).
    fn is_in_game(&self) -> bool {
        self.category() == StateCategory::Playing
    }

    /// Whether this state has a world background (Playing, Paused, or any
    /// subscreen whose previous state has a world background).
    fn has_world_background(&self) -> bool;

    /// Return the parent state for stacked subscreens, or `None` for top-level states.
    fn previous_state(&self) -> Option<&GameState>;

    /// Return the owned parent state for stacked subscreens, consuming self.
    fn into_previous_state(self) -> Option<GameState>;

    /// Whether a transition to the target state is valid from the current state.
    /// This encodes the state machine's transition rules.
    fn can_transition_to(&self, target: &Self) -> bool;

    /// Perform a validated state transition. Returns `Err(current)` if the
    /// transition is invalid, allowing the caller to recover the original state.
    fn transition(self, target: Self) -> Result<Self, Self>
    where
        Self: Sized,
    {
        if self.can_transition_to(&target) {
            Ok(target)
        } else {
            Err(self)
        }
    }
}

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

    /// Whether this state is a subscreen (has a `previous` field).
    pub fn is_subscreen(&self) -> bool {
        self.previous_state().is_some()
    }

    /// Whether the state represents an active gameplay state (not menus, not loading).
    pub fn is_playing(&self) -> bool {
        matches!(self, GameState::Playing)
    }

    /// Whether the state is in the connection lifecycle (Connecting or LoadingWorld).
    pub fn is_connecting(&self) -> bool {
        matches!(self, GameState::Connecting | GameState::LoadingWorld)
    }

    /// Create a subscreen state that stacks on top of the current state.
    pub fn open_options(self) -> GameState {
        GameState::Options {
            previous: Box::new(self),
        }
    }

    pub fn open_video_settings(self) -> GameState {
        GameState::VideoSettings {
            previous: Box::new(self),
        }
    }

    pub fn open_controls(self) -> GameState {
        GameState::Controls {
            previous: Box::new(self),
        }
    }

    pub fn open_skin_customization(self) -> GameState {
        GameState::SkinCustomization {
            previous: Box::new(self),
        }
    }

    pub fn open_language(self) -> GameState {
        GameState::Language {
            previous: Box::new(self),
        }
    }

    pub fn open_audio_settings(self) -> GameState {
        GameState::AudioSettings {
            previous: Box::new(self),
        }
    }

    pub fn open_chat_settings(self) -> GameState {
        GameState::ChatSettings {
            previous: Box::new(self),
        }
    }

    pub fn open_resource_packs(self) -> GameState {
        GameState::ResourcePacks {
            previous: Box::new(self),
        }
    }

    pub fn open_shader_packs(self) -> GameState {
        GameState::ShaderPacks {
            previous: Box::new(self),
        }
    }

    pub fn open_modding(self) -> GameState {
        GameState::Modding {
            previous: Box::new(self),
        }
    }

    pub fn open_mod_config(self, mod_id: String) -> GameState {
        GameState::ModConfig {
            previous: Box::new(self),
            mod_id,
        }
    }
}

impl GameStateMachine for GameState {
    fn category(&self) -> StateCategory {
        match self {
            GameState::MainMenu
            | GameState::AltManager
            | GameState::Multiplayer
            | GameState::DirectConnect
            | GameState::ServerEditor { .. } => StateCategory::Menu,
            GameState::Playing => StateCategory::Playing,
            GameState::Paused => StateCategory::Paused,
            GameState::Connecting => StateCategory::Connecting,
            GameState::LoadingWorld => StateCategory::LoadingWorld,
            GameState::Disconnected { .. } => StateCategory::Disconnected,
            GameState::Options { .. }
            | GameState::VideoSettings { .. }
            | GameState::Controls { .. }
            | GameState::SkinCustomization { .. }
            | GameState::Language { .. }
            | GameState::AudioSettings { .. }
            | GameState::ChatSettings { .. }
            | GameState::ResourcePacks { .. }
            | GameState::ShaderPacks { .. }
            | GameState::Modding { .. }
            | GameState::ModConfig { .. } => StateCategory::Subscreen,
        }
    }

    fn has_world_background(&self) -> bool {
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

    fn previous_state(&self) -> Option<&GameState> {
        match self {
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
            | GameState::ModConfig { previous, .. } => Some(previous),
            _ => None,
        }
    }

    fn into_previous_state(self) -> Option<GameState> {
        match self {
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
            | GameState::ModConfig { previous, .. } => Some(*previous),
            _ => None,
        }
    }

    fn can_transition_to(&self, target: &Self) -> bool {
        let from_cat = self.category();
        let to_cat = target.category();

        match (from_cat, to_cat) {
            // From Menu: can go to any other menu screen, connecting, or subscreen
            (StateCategory::Menu, StateCategory::Menu) => true,
            (StateCategory::Menu, StateCategory::Connecting) => true,
            (StateCategory::Menu, StateCategory::Subscreen) => true,

            // From Connecting: can go to LoadingWorld, Disconnected, or back to Menu
            (StateCategory::Connecting, StateCategory::LoadingWorld) => true,
            (StateCategory::Connecting, StateCategory::Disconnected) => true,
            (StateCategory::Connecting, StateCategory::Menu) => true,

            // From LoadingWorld: can go to Playing, Disconnected, or back to Menu
            (StateCategory::LoadingWorld, StateCategory::Playing) => true,
            (StateCategory::LoadingWorld, StateCategory::Disconnected) => true,
            (StateCategory::LoadingWorld, StateCategory::Menu) => true,

            // From Playing: can go to Paused, Disconnected, or Subscreen
            (StateCategory::Playing, StateCategory::Paused) => true,
            (StateCategory::Playing, StateCategory::Disconnected) => true,
            (StateCategory::Playing, StateCategory::Subscreen) => true,
            (StateCategory::Playing, StateCategory::Menu) => true,

            // From Paused: can go to Playing, Subscreen, Menu, or Disconnected
            (StateCategory::Paused, StateCategory::Playing) => true,
            (StateCategory::Paused, StateCategory::Subscreen) => true,
            (StateCategory::Paused, StateCategory::Menu) => true,
            (StateCategory::Paused, StateCategory::Disconnected) => true,

            // From Subscreen: can go to parent (via into_previous), another subscreen,
            // Playing, Paused, or Menu
            (StateCategory::Subscreen, StateCategory::Subscreen) => true,
            (StateCategory::Subscreen, StateCategory::Playing) => true,
            (StateCategory::Subscreen, StateCategory::Paused) => true,
            (StateCategory::Subscreen, StateCategory::Menu) => true,
            (StateCategory::Subscreen, StateCategory::Disconnected) => true,

            // From Disconnected: can go to Menu or Connecting
            (StateCategory::Disconnected, StateCategory::Menu) => true,
            (StateCategory::Disconnected, StateCategory::Connecting) => true,

            _ => false,
        }
    }
}
