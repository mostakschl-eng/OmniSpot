#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum WarningContent {
    Elevated,
    KeyEventsStealing,
}

impl WarningContent {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Elevated => "Elevated application",
            Self::KeyEventsStealing => "Restricted application",
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Elevated => {
                "OmniSpot can no longer detect your keystrokes while an elevated app is in focus."
            }
            Self::KeyEventsStealing => {
                "OmniSpot cannot detect your keystrokes while this app is in focus."
            }
        }
    }

    pub(crate) fn learn_more_url(self) -> Option<&'static str> {
        match self {
            Self::Elevated => Some("https://app-switcher.com/elevated-apps/"),
            Self::KeyEventsStealing => None,
        }
    }
}
