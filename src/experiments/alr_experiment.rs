// WebRTC-ProbingScreenshareBwe
// Apparently these are now unconfigurable constants?
#[derive(Clone, Debug)]
pub struct AlrExperimentSettings {
    pub pacing_factor: f64,
    pub max_paced_queue_time: i64,
    pub alr_bandwidth_usage_percent: isize,
    pub alr_start_budget_level_percent: isize,
    pub alr_stop_budget_level_percent: isize,
    // Will be sent to the receive side for stats slicing.
    // Can be 0..6, because it's sent as a 3 bits value and there's also
    // reserved value to indicate absence of experiment.
    pub group_id: isize,
}

impl Default for AlrExperimentSettings {
    fn default() -> Self {
        Self {
            pacing_factor: 1.0,
            max_paced_queue_time: 2875,
            alr_bandwidth_usage_percent: 80,
            alr_start_budget_level_percent: 40,
            alr_stop_budget_level_percent: -60,
            group_id: 3,
        }
    }
}
