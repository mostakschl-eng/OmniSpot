#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultType {
    App,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub target: String,
    pub result_type: SearchResultType,
    pub score: i32,
    pub icon_path: Option<String>,
}

impl SearchResult {
    pub fn app(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        target: impl Into<String>,
        score: i32,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            target: target.into(),
            result_type: SearchResultType::App,
            score,
            icon_path: None,
        }
    }

    pub fn app_with_icon(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        target: impl Into<String>,
        score: i32,
        icon_path: impl Into<String>,
    ) -> Self {
        let mut result = Self::app(title, subtitle, target, score);
        result.icon_path = Some(icon_path.into());
        result
    }

    pub fn file(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        target: impl Into<String>,
        score: i32,
        icon_path: Option<String>,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            target: target.into(),
            result_type: SearchResultType::File,
            score,
            icon_path,
        }
    }
}
