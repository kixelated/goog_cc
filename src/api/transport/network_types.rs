use std::cmp::Ordering;

use crate::api::units::{DataRate, DataSize, Timestamp};

use super::EcnMarking;

pub struct PacedPacketInfo {
    pub send_bitrate: DataRate,
    pub probe_cluster_id: isize,
    pub probe_cluster_min_probes: isize,
    pub probe_cluster_min_bytes: isize,
    pub probe_cluster_bytes_sent: isize,
  }

  impl PacedPacketInfo {
    pub const NotAProbe: isize = -1;

    pub fn new(probe_cluster_id: isize, probe_cluster_min_probes: isize, probe_cluster_min_bytes: isize) -> Self {
      Self {
        send_bitrate: DataRate::BitsPerSec(0),
        probe_cluster_id,
        probe_cluster_min_probes,
        probe_cluster_min_bytes,
        probe_cluster_bytes_sent: 0,
      }
    }
}

  impl Default for PacedPacketInfo {
    fn default() -> Self {
      Self {
        send_bitrate: DataRate::BitsPerSec(0),
        probe_cluster_id:Self::NotAProbe,
        probe_cluster_min_probes: -1,
        probe_cluster_min_bytes: -1,
        probe_cluster_bytes_sent: 0,
      }
    }
  }

  impl PartialEq for PacedPacketInfo {
    fn eq(&self, other: &Self) -> bool {
      self.send_bitrate == other.send_bitrate &&
      self.probe_cluster_id == other.probe_cluster_id &&
      self.probe_cluster_min_probes == other.probe_cluster_min_probes &&
      self.probe_cluster_min_bytes == other.probe_cluster_min_bytes
    }
  }

pub struct SentPacket {
    pub send_time: Timestamp,
    // Size of packet with overhead up to IP layer.
    pub size: DataSize,
    // Size of preceeding packets that are not part of feedback.
    pub prior_unacked_data: DataSize,
    // Probe cluster id and parameters including bitrate, number of packets and
    // number of bytes.
    pub pacing_info: PacedPacketInfo,
    // True if the packet is an audio packet, false for video, padding, RTX etc.
    pub audio: bool,
    // Transport independent sequence number, any tracked packet should have a
    // sequence number that is unique over the whole call and increasing by 1 for
    // each packet.
    pub sequence_number: i64,
    // Tracked data in flight when the packet was sent, excluding unacked data.
    pub data_in_flight: DataSize,
  }

  impl Default for SentPacket {
    fn default() -> Self {
      Self {
        send_time: Timestamp::PlusInfinity(),
        size: DataSize::Zero(),
        prior_unacked_data: DataSize::Zero(),
        pacing_info: PacedPacketInfo::default(),
        audio: false,
        sequence_number: 0,
        data_in_flight: DataSize::Zero(),
      }
    }
  }

pub struct PacketResult {
    pub sent_packet: SentPacket,
    pub receive_time: Timestamp,
    pub ecn: EcnMarking,
  }

  impl PacketResult {
    pub const fn IsReceived(&self) -> bool { !self.receive_time.IsPlusInfinity()}
  }

  impl Default for PacketResult  {
    fn default() -> Self {
      Self {
        sent_packet: SentPacket::default(),
        receive_time: Timestamp::PlusInfinity(),
        ecn: EcnMarking::NotEct,
      }
    }
  }

  impl PartialOrd for PacketResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.receive_time != other.receive_time {
          return Some(self.receive_time.cmp(&other.receive_time));
        }

        if self.sent_packet.send_time != other.sent_packet.send_time {
          return Some(self.sent_packet.send_time.cmp(&other.sent_packet.send_time));
        }

        Some(self.sent_packet.sequence_number.cmp(&other.sent_packet.sequence_number))
    }
  }

  impl PartialEq for PacketResult {
    fn eq(&self, other: &Self) -> bool {
        self.receive_time == other.receive_time &&
        self.sent_packet.send_time == other.sent_packet.send_time &&
        self.sent_packet.sequence_number == other.sent_packet.sequence_number
    }
  }
