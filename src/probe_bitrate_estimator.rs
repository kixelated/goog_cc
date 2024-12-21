/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::BTreeMap;

use crate::api::{
    transport::PacketResult,
    units::{DataRate, DataSize, Timestamp},
};

struct AggregatedCluster {
    pub num_probes: isize,
    pub first_send: Timestamp,
    pub last_send: Timestamp,
    pub first_receive: Timestamp,
    pub last_receive: Timestamp,
    pub size_last_send: DataSize,
    pub size_first_receive: DataSize,
    pub size_total: DataSize,
}

impl Default for AggregatedCluster {
    fn default() -> Self {
        Self {
            num_probes: 0,
            first_send: Timestamp::PlusInfinity(),
            last_send: Timestamp::MinusInfinity(),
            first_receive: Timestamp::PlusInfinity(),
            last_receive: Timestamp::MinusInfinity(),
            size_last_send: DataSize::Zero(),
            size_first_receive: DataSize::Zero(),
            size_total: DataSize::Zero(),
        }
    }
}

pub struct ProbeBitrateEstimator {
    clusters: BTreeMap<isize, AggregatedCluster>,
    estimated_data_rate: Option<DataRate>,
}

impl ProbeBitrateEstimator {
    // Should be called for every probe packet we receive feedback about.
    // Returns the estimated bitrate if the probe completes a valid cluster.
    pub fn HandleProbeAndEstimateBitrate(
        &mut self,
        packet_feedback: &PacketResult,
    ) -> Option<DataRate> {
        todo!();
    }

    pub fn FetchAndResetLastEstimatedBitrate(&mut self) -> Option<DataRate> {
        todo!();
    }

    // Erases old cluster data that was seen before `timestamp`.
    fn EraseOldClusters(&mut self, timestamp: Timestamp) {
        todo!();
    }
}
