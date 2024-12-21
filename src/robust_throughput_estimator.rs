/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::VecDeque;

use crate::{
    api::{
        transport::PacketResult,
        units::{DataRate, Timestamp},
    },
    AcknowledgedBitrateEstimatorInterface, RobustThroughputEstimatorSettings,
};

pub struct RobustThroughputEstimator {
    settings: RobustThroughputEstimatorSettings,
    window: VecDeque<PacketResult>,
    latest_discarded_send_time: Timestamp,
}

impl Default for RobustThroughputEstimator {
    fn default() -> Self {
        Self {
            settings: RobustThroughputEstimatorSettings::default(),
            window: VecDeque::new(),
            latest_discarded_send_time: Timestamp::MinusInfinity(),
        }
    }
}

impl RobustThroughputEstimator {
    fn FirstPacketOutsideWindow() -> bool {
        todo!();
    }
}

impl AcknowledgedBitrateEstimatorInterface for RobustThroughputEstimator {
    fn incoming_packet_feedback(&mut self, packet_feedback_vector: &[PacketResult]) {
        todo!();
    }

    fn bitrate(&self) -> Option<DataRate> {
        todo!()
    }

    fn peek_rate(&self) -> Option<DataRate> {
        todo!()
    }

    fn set_alr(&mut self, in_alr: bool) {
        todo!()
    }

    fn set_alr_ended_time(&mut self, alr_ended_time: crate::api::units::Timestamp) {
        todo!()
    }
}
