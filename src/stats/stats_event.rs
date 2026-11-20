#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Keep the names of the corresponding C# event records.
pub(crate) enum StatsEvent {
    SwitchEvent {
        process_name: String,
        process_id: Option<u32>,
        process_path: String,
        total_choices: i32,
        modifier_down_tick: i64,
        letter_down_tick: i64,
        previous_letter_up_tick: Option<i64>,
        is_dynamic: bool,
        trigger_key: u32,
    },
    PeekEvent {
        target_process_name: String,
        target_process_path: String,
        arm_tick: i64,
        finish_tick: i64,
        is_dynamic: bool,
    },
    AltTabEvent(i32),
}
