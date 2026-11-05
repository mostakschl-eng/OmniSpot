#[derive(Default)]
pub struct SpotlightState {
    pub query: String,
    pub suggestions: Vec<SearchResult>,
    pub results: Vec<SearchResult>,
    pub selected_path: String,
}

pub struct SearchResult {
    pub title: String,
    pub kind: String,
}
