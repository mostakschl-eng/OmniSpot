#[derive(Default)]
pub struct SpotlightState {
    pub query: String,
    pub suggestions: Vec<SearchResult>,
    pub results: Vec<SearchResult>,
    pub selected_path: String,
    pub selected_index: usize,
}

#[derive(Clone)]
pub struct SearchResult {
    pub title: String,
    pub kind: String,
    pub target: String,
    pub icon_path: Option<String>,
}

impl SpotlightState {
    pub fn handle_key(&mut self, key: &str, typed: Option<&str>) -> bool {
        match key.to_ascii_lowercase().as_str() {
            "backspace" => {
                self.query.pop();
                self.selected_index = 0;
                true
            }
            "up" => {
                self.move_selection(-1);
                true
            }
            "down" => {
                self.move_selection(1);
                true
            }
            "escape" => true,
            "enter" => true,
            _ => {
                if let Some(value) = typed.filter(|value| !value.is_empty()) {
                    self.query.push_str(value);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let count = if self.query.is_empty() {
            self.suggestions.len()
        } else {
            self.results.len()
        };
        if count == 0 {
            return;
        }
        let current = self.selected_index as i32;
        let next = (current + delta).rem_euclid(count as i32) as usize;
        self.selected_index = next;
        self.selected_path = if self.query.is_empty() {
            self.suggestions
                .get(next)
                .map(|result| result.target.clone())
                .unwrap_or_default()
        } else {
            self.results
                .get(next)
                .map(|result| result.target.clone())
                .unwrap_or_default()
        };
    }
}
