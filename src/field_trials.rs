use crate::{experiments::{AlrExperimentSettings, CongestionWindowConfig, VideoRateControlConfig}, remote_bitrate_estimator::{BweBackOffFactor, EstimateBoundedIncrease}, AlrDetectorConfig, BweSeparateAudioPacketsSettings, LossBasedBweV2Config, LossBasedControlConfig, ProbeControllerConfig, RobustThroughputEstimatorSettings, RttBasedBackoffConfig, TrendlineEstimatorSettings};

#[derive(Default, Clone, Debug)]
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
}

impl FieldTrials {
    pub fn parse(trials: &str) -> Self {
        todo!()
    }
}
