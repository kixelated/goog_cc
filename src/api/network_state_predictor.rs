use super::transport::BandwidthUsage;

pub trait NetworkStatePredictor {
    // Returns current network state prediction.
    // Inputs:  send_time_ms - packet send time.
    //          arrival_time_ms - packet arrival time.
    //          network_state - computed network state.
    fn Update(
        &mut self,
        send_time_ms: i64,
        arrival_time_ms: i64,
        network_state: BandwidthUsage,
    ) -> BandwidthUsage;
}
