use crate::{
    experiments::{AlrExperimentSettings, CongestionWindowConfig, VideoRateControlConfig},
    remote_bitrate_estimator::{BweBackOffFactor, EstimateBoundedIncrease},
    AlrDetectorConfig, BitrateEstimatorConfig, BweLossExperiment, BweSeparateAudioPacketsSettings,
    LossBasedBweV2Config, LossBasedControlConfig, ProbeControllerConfig,
    RobustThroughputEstimatorSettings, RttBasedBackoffConfig, SafeResetOnRouteChange,
    TrendlineEstimatorSettings,
};

#[derive(Clone, Debug, Default)]
pub struct FieldTrials {
    // WebRTC-Bwe-RobustThroughputEstimatorSettings
    pub robust_throughput_estimator_settings: RobustThroughputEstimatorSettings,

    // Probing settings.
    // WebRTC-Bwe-ProbingConfiguration
    pub probing_configuration: ProbeControllerConfig,

    // Use probing to recover faster after large bitrate estimate drops.
    // WebRTC-BweRapidRecoveryExperiment
    pub rapid_recovery_experiment: bool,

    // WebRTC-AlrDetectorParameters
    pub alr_detector_parameters: AlrDetectorConfig,

    // WebRTC-ProbingScreenshareBwe or WebRTC-StrictPacingAndProbing (legacy?)
    pub alr_experiment_settings: AlrExperimentSettings,

    // WebRTC-AddPacingToCongestionWindowPushback
    pub add_pacing_to_congestion_window_pushback: bool,

    // WebRTC-CongestionWindow
    pub congestion_window: CongestionWindowConfig,

    // WebRTC-VideoRateControl
    pub video_rate_control: VideoRateControlConfig,

    // WebRTC-UseBaseHeavyVP8TL3RateAllocation
    pub vp8_base_heavy_tl3_alloc: bool,

    // WebRTC-Bwe-SeparateAudioPackets
    pub separate_audio_packets: BweSeparateAudioPacketsSettings,

    // WebRTC-Bwe-LossBasedControl
    pub loss_based_control: LossBasedControlConfig,

    // LossBasedBweV2Config
    pub loss_based_bwe_v2: LossBasedBweV2Config,

    // WebRTC-Bwe-MaxRttLimit
    pub max_rtt_limit: RttBasedBackoffConfig,

    // WebRTC-Bwe-TrendlineEstimatorSettings
    pub trendline_estimator_settings: TrendlineEstimatorSettings,

    // WebRTC-BweWindowSizeInPackets
    // TODO: Legacy? TrendlineEstimatorSettings takes priority.
    // pub window_size_in_packets: BweWindowSizeInPackets, // Enabled-*

    // WebRTC-BweBackOffFactor
    pub bwe_back_off_factor: BweBackOffFactor,

    // WebRTC-DontIncreaseDelayBasedBweInAlr
    pub no_bitrate_increase_in_alr: bool,

    // WebRTC-Bwe-EstimateBoundedIncrease
    pub estimate_bounded_increase: EstimateBoundedIncrease,

    // WebRTC-Bwe-MinAllocAsLowerBound
    pub min_alloc_as_lower_bound: Option<bool>,

    // WebRTC-Bwe-IgnoreProbesLowerThanNetworkStateEstimate
    pub ignore_probes_lower_than_network_state_estimate: Option<bool>,

    // WebRTC-Bwe-LimitProbesLowerThanThroughputEstimate
    pub limit_probes_lower_than_throughput_estimate: Option<bool>,

    // WebRTC-Bwe-PaceAtMaxOfBweAndLowerLinkCapacity
    pub pace_at_max_of_bwe_and_lower_link_capacity: Option<bool>,

    // WebRTC-Bwe-LimitPacingFactorByUpperLinkCapacityEstimate
    pub limit_pacing_factor_by_upper_link_capacity_estimate: Option<bool>,

    // WebRTC-Bwe-SafeResetOnRouteChange
    pub safe_reset_on_route_change: SafeResetOnRouteChange,

    // WebRTC-BweThroughputWindowConfig
    pub bwe_throughput_window_config: BitrateEstimatorConfig,

    // WebRTC-BweLossExperiment
    pub loss_experiment: BweLossExperiment,

    // WebRTC-Bwe-ReceiverLimitCapsOnly
    pub receiver_limit_caps_only: bool,
}

impl FieldTrials {
    pub fn parse(trials: &str) -> Self {
        todo!()
    }
}
