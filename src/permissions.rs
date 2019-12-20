use std::collections::HashMap;

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

    /// Returns highest possible permission level
    pub fn highest() -> PermissionLevel {
        PermissionLevel::Admin
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

#[cfg(test)]
mod tests {
    use super::*;

    fn exhaustive_list_of_variants() -> Vec<PermissionLevel> {
        let variants = vec![
            PermissionLevel::User,
            PermissionLevel::Admin
        ];
        for var in variants.iter() {
            match var {
                PermissionLevel::Admin => assert!(true),
                PermissionLevel::User => assert!(true),
                #[allow(unreachable_patterns)]
                _ => assert!(false, "not all enum variants are tested"),
            }
        }
        variants
    }

    #[test]
    fn test_lowest_is_lowest() {
        let lowest = PermissionLevel::lowest();
        for level in exhaustive_list_of_variants().iter() {
            assert!(level.permits(lowest),
                    "{:?} is lowest, but is not permitted by {:?}", lowest, level);
        }
    }

    #[test]
    fn test_highest_is_highest() {
        let highest = PermissionLevel::highest();
        for level in exhaustive_list_of_variants().iter() {
            assert!(highest.permits(*level),
                    "{:?} is highest, but does not permit {:?}", highest, level);
        }
    }
}