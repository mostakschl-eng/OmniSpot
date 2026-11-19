pub(crate) trait ISessionStats {
    fn record_switch(
        &mut self,
        process_name: &str,
        previous_process_name: Option<&str>,
        duration_ms: i32,
        saved_ms: i32,
        is_dynamic: bool,
        fastest_duration_ms: Option<i32>,
        trigger_key: &str,
    );
    fn record_peek(&mut self, process_name: &str, duration_ms: i32, is_dynamic: bool);
    fn record_alt_tab(&mut self, tab_count: i32);
}
