use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppThemeSetting {
    #[default]
    System,
    Dark,
    Light,
}
