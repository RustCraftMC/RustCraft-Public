use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    #[serde(rename = "client.read")]
    ClientRead,
    #[serde(rename = "client.modify")]
    ClientModify,
    #[serde(rename = "ui.read")]
    UiRead,
    #[serde(rename = "ui.modify")]
    UiModify,
    #[serde(rename = "input.observe")]
    InputObserve,
    #[serde(rename = "input.consume")]
    InputConsume,
    #[serde(rename = "render.read")]
    RenderRead,
    #[serde(rename = "render.modify")]
    RenderModify,
    #[serde(rename = "render.custom_draw")]
    RenderCustomDraw,
    #[serde(rename = "animation.read")]
    AnimationRead,
    #[serde(rename = "animation.modify")]
    AnimationModify,
    #[serde(rename = "resources.read")]
    ResourcesRead,
    #[serde(rename = "resources.register")]
    ResourcesRegister,
    #[serde(rename = "network.observe")]
    NetworkObserve,
    #[serde(rename = "network.modify")]
    NetworkModify,
    #[serde(rename = "network.cancel")]
    NetworkCancel,
    #[serde(rename = "network.send")]
    NetworkSend,
    #[serde(rename = "protocol.inspect")]
    ProtocolInspect,
    #[serde(rename = "protocol.translate")]
    ProtocolTranslate,
    #[serde(rename = "storage.read")]
    StorageRead,
    #[serde(rename = "storage.write")]
    StorageWrite,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClientRead => "client.read",
            Self::ClientModify => "client.modify",
            Self::UiRead => "ui.read",
            Self::UiModify => "ui.modify",
            Self::InputObserve => "input.observe",
            Self::InputConsume => "input.consume",
            Self::RenderRead => "render.read",
            Self::RenderModify => "render.modify",
            Self::RenderCustomDraw => "render.custom_draw",
            Self::AnimationRead => "animation.read",
            Self::AnimationModify => "animation.modify",
            Self::ResourcesRead => "resources.read",
            Self::ResourcesRegister => "resources.register",
            Self::NetworkObserve => "network.observe",
            Self::NetworkModify => "network.modify",
            Self::NetworkCancel => "network.cancel",
            Self::NetworkSend => "network.send",
            Self::ProtocolInspect => "protocol.inspect",
            Self::ProtocolTranslate => "protocol.translate",
            Self::StorageRead => "storage.read",
            Self::StorageWrite => "storage.write",
        }
    }

    pub fn is_sensitive(self) -> bool {
        matches!(
            self,
            Self::NetworkModify
                | Self::NetworkCancel
                | Self::NetworkSend
                | Self::ProtocolTranslate
                | Self::ResourcesRegister
        )
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionSet {
    requested: HashSet<Permission>,
    granted: HashSet<Permission>,
}

impl PermissionSet {
    pub fn resolve(mod_id: &str, requested: &[Permission], policy: &PermissionPolicy) -> Self {
        let requested: HashSet<_> = requested.iter().copied().collect();
        let granted = requested
            .iter()
            .copied()
            .filter(|permission| !permission.is_sensitive() || policy.allows(mod_id, *permission))
            .collect();
        Self { requested, granted }
    }

    pub fn contains(&self, permission: Permission) -> bool {
        self.granted.contains(&permission)
    }

    pub fn denied(&self) -> impl Iterator<Item = Permission> + '_ {
        self.requested.difference(&self.granted).copied()
    }

    pub fn granted(&self) -> impl Iterator<Item = Permission> + '_ {
        self.granted.iter().copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionPolicy {
    approved_sensitive: HashMap<String, HashSet<Permission>>,
}

impl PermissionPolicy {
    pub fn approve_for(&mut self, mod_id: impl Into<String>, permission: Permission) {
        if permission.is_sensitive() {
            self.approved_sensitive
                .entry(mod_id.into())
                .or_default()
                .insert(permission);
        }
    }

    pub fn allows(&self, mod_id: &str, permission: Permission) -> bool {
        self.approved_sensitive
            .get(mod_id)
            .is_some_and(|permissions| permissions.contains(&permission))
    }

    pub fn load(path: &std::path::Path) -> Self {
        let Ok(json) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(entries) = serde_json::from_str::<HashMap<String, Vec<Permission>>>(&json) else {
            log::warn!(
                target: "rustcraft::lua",
                "ignoring invalid permission policy at {}",
                path.display()
            );
            return Self::default();
        };
        let mut policy = Self::default();
        for (mod_id, permissions) in entries {
            for permission in permissions {
                policy.approve_for(mod_id.clone(), permission);
            }
        }
        policy
    }
}

pub fn permission_for_event(name: &str) -> Option<Permission> {
    if name.starts_with("animation.") {
        Some(Permission::AnimationModify)
    } else if name.starts_with("render.") {
        Some(Permission::RenderRead)
    } else if name.starts_with("ui.") {
        Some(Permission::UiRead)
    } else if name.starts_with("input.") {
        Some(Permission::InputObserve)
    } else if name.starts_with("network.") {
        Some(Permission::NetworkObserve)
    } else if name.starts_with("protocol.") {
        Some(Permission::ProtocolInspect)
    } else if name.starts_with("client.") {
        Some(Permission::ClientRead)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_permissions_are_denied_by_default() {
        let set = PermissionSet::resolve(
            "test",
            &[Permission::NetworkObserve, Permission::NetworkSend],
            &PermissionPolicy::default(),
        );
        assert!(set.contains(Permission::NetworkObserve));
        assert!(!set.contains(Permission::NetworkSend));
        assert_eq!(
            set.denied().collect::<Vec<_>>(),
            vec![Permission::NetworkSend]
        );
    }

    #[test]
    fn sensitive_approval_is_scoped_to_one_mod() {
        let mut policy = PermissionPolicy::default();
        policy.approve_for("trusted", Permission::NetworkModify);
        assert!(
            PermissionSet::resolve("trusted", &[Permission::NetworkModify], &policy)
                .contains(Permission::NetworkModify)
        );
        assert!(
            !PermissionSet::resolve("other", &[Permission::NetworkModify], &policy)
                .contains(Permission::NetworkModify)
        );
    }
}
