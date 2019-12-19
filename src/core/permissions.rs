use crate::core::model::HashMap;

#[derive(Debug, Clone, Copy)]
pub enum PermissionLevel {
    Admin = 100,
    User = 10,
}

impl PermissionLevel {
    /// Checks whether this level of permission allows actions with specified permission level.
    pub fn permits(&self, other: PermissionLevel) -> bool {
        *self as i32 >= other as i32
    }

    /// Returns lowest possible permission level
    pub fn lowest() -> PermissionLevel {
        PermissionLevel::User
    }
}

pub struct PermissionList {
    permissions: HashMap<String, PermissionLevel>,
}

impl PermissionList {
    pub fn new(permissions: HashMap<String, PermissionLevel>) -> PermissionList {
        PermissionList { permissions }
    }

    pub fn get(&self, key: &str) -> PermissionLevel {
        *self.permissions.get(key).unwrap_or(&PermissionLevel::lowest())
    }
}
