/*
 *  Copyright (c) 2013 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

const fn IsNewer16(value: u16, prev_value: u16) -> bool {
    // kBreakpoint is the half-way mark for the type U. For instance, for a
    // uint16_t it will be 0x8000, and for a uint32_t, it will be 0x8000000.
    const Breakpoint: u16 = (u16::MAX >> 1) + 1;
    // Distinguish between elements that are exactly kBreakpoint apart.
    // If t1>t2 and |t1-t2| = kBreakpoint: IsNewer(t1,t2)=true,
    // IsNewer(t2,t1)=false
    // rather than having IsNewer(t1,t2) = IsNewer(t2,t1) = false.
    match value.wrapping_sub(prev_value) {
        1..Breakpoint => true,
        Breakpoint => value > prev_value,
        _ => false,
    }
}

const fn IsNewer32(value: u32, prev_value: u32) -> bool {
    // kBreakpoint is the half-way mark for the type U. For instance, for a
    // uint16_t it will be 0x8000, and for a uint32_t, it will be 0x8000000.
    const Breakpoint: u32 = (u32::MAX >> 1) + 1;
    // Distinguish between elements that are exactly kBreakpoint apart.
    // If t1>t2 and |t1-t2| = kBreakpoint: IsNewer(t1,t2)=true,
    // IsNewer(t2,t1)=false
    // rather than having IsNewer(t1,t2) = IsNewer(t2,t1) = false.
    match value.wrapping_sub(prev_value) {
        1..Breakpoint => true,
        Breakpoint => value > prev_value,
        _ => false,
    }
}

// NB: Doesn't fulfill strict weak ordering requirements.
//     Mustn't be used as std::map Compare function.
pub const fn IsNewerSequenceNumber(sequence_number: u16, prev_sequence_number: u16) -> bool {
    return IsNewer16(sequence_number, prev_sequence_number);
}

// NB: Doesn't fulfill strict weak ordering requirements.
//     Mustn't be used as std::map Compare function.
pub const fn IsNewerTimestamp(timestamp: u32, prev_timestamp: u32) -> bool {
    return IsNewer32(timestamp, prev_timestamp);
}

pub const fn LatestSequenceNumber(sequence_number1: u16, sequence_number2: u16) -> u16 {
    if IsNewerSequenceNumber(sequence_number1, sequence_number2) {
        sequence_number1
    } else {
        sequence_number2
    }
}

pub const fn LatestTimestamp(timestamp1: u32, timestamp2: u32) -> u32 {
    if IsNewerTimestamp(timestamp1, timestamp2) {
        timestamp1
    } else {
        timestamp2
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn IsNewerSequenceNumberEqual() {
        assert!(!IsNewerSequenceNumber(0x0001, 0x0001));
    }

    #[test]
    fn IsNewerSequenceNumberNoWrap() {
        assert!(IsNewerSequenceNumber(0xFFFF, 0xFFFE));
        assert!(IsNewerSequenceNumber(0x0001, 0x0000));
        assert!(IsNewerSequenceNumber(0x0100, 0x00FF));
    }

    #[test]
    fn IsNewerSequenceNumberForwardWrap() {
        assert!(IsNewerSequenceNumber(0x0000, 0xFFFF));
        assert!(IsNewerSequenceNumber(0x0000, 0xFF00));
        assert!(IsNewerSequenceNumber(0x00FF, 0xFFFF));
        assert!(IsNewerSequenceNumber(0x00FF, 0xFF00));
    }

    #[test]
    fn IsNewerSequenceNumberBackwardWrap() {
        assert!(!IsNewerSequenceNumber(0xFFFF, 0x0000));
        assert!(!IsNewerSequenceNumber(0xFF00, 0x0000));
        assert!(!IsNewerSequenceNumber(0xFFFF, 0x00FF));
        assert!(!IsNewerSequenceNumber(0xFF00, 0x00FF));
    }

    #[test]
    fn IsNewerSequenceNumberHalfWayApart() {
        assert!(IsNewerSequenceNumber(0x8000, 0x0000));
        assert!(!IsNewerSequenceNumber(0x0000, 0x8000));
    }

    #[test]
    fn IsNewerTimestampEqual() {
        assert!(!IsNewerTimestamp(0x00000001, 0x000000001));
    }

    #[test]
    fn IsNewerTimestampNoWrap() {
        assert!(IsNewerTimestamp(0xFFFFFFFF, 0xFFFFFFFE));
        assert!(IsNewerTimestamp(0x00000001, 0x00000000));
        assert!(IsNewerTimestamp(0x00010000, 0x0000FFFF));
    }

    #[test]
    fn IsNewerTimestampForwardWrap() {
        assert!(IsNewerTimestamp(0x00000000, 0xFFFFFFFF));
        assert!(IsNewerTimestamp(0x00000000, 0xFFFF0000));
        assert!(IsNewerTimestamp(0x0000FFFF, 0xFFFFFFFF));
        assert!(IsNewerTimestamp(0x0000FFFF, 0xFFFF0000));
    }

    #[test]
    fn IsNewerTimestampBackwardWrap() {
        assert!(!IsNewerTimestamp(0xFFFFFFFF, 0x00000000));
        assert!(!IsNewerTimestamp(0xFFFF0000, 0x00000000));
        assert!(!IsNewerTimestamp(0xFFFFFFFF, 0x0000FFFF));
        assert!(!IsNewerTimestamp(0xFFFF0000, 0x0000FFFF));
    }

    #[test]
    fn IsNewerTimestampHalfWayApart() {
        assert!(IsNewerTimestamp(0x80000000, 0x00000000));
        assert!(!IsNewerTimestamp(0x00000000, 0x80000000));
    }

    #[test]
    fn LatestSequenceNumberNoWrap() {
        assert_eq!(0xFFFF, LatestSequenceNumber(0xFFFF, 0xFFFE));
        assert_eq!(0x0001, LatestSequenceNumber(0x0001, 0x0000));
        assert_eq!(0x0100, LatestSequenceNumber(0x0100, 0x00FF));

        assert_eq!(0xFFFF, LatestSequenceNumber(0xFFFE, 0xFFFF));
        assert_eq!(0x0001, LatestSequenceNumber(0x0000, 0x0001));
        assert_eq!(0x0100, LatestSequenceNumber(0x00FF, 0x0100));
    }

    #[test]
    fn LatestSequenceNumberWrap() {
        assert_eq!(0x0000, LatestSequenceNumber(0x0000, 0xFFFF));
        assert_eq!(0x0000, LatestSequenceNumber(0x0000, 0xFF00));
        assert_eq!(0x00FF, LatestSequenceNumber(0x00FF, 0xFFFF));
        assert_eq!(0x00FF, LatestSequenceNumber(0x00FF, 0xFF00));

        assert_eq!(0x0000, LatestSequenceNumber(0xFFFF, 0x0000));
        assert_eq!(0x0000, LatestSequenceNumber(0xFF00, 0x0000));
        assert_eq!(0x00FF, LatestSequenceNumber(0xFFFF, 0x00FF));
        assert_eq!(0x00FF, LatestSequenceNumber(0xFF00, 0x00FF));
    }

    #[test]
    fn LatestTimestampNoWrap() {
        assert_eq!(0xFFFFFFFF, LatestTimestamp(0xFFFFFFFF, 0xFFFFFFFE));
        assert_eq!(0x00000001, LatestTimestamp(0x00000001, 0x00000000));
        assert_eq!(0x00010000, LatestTimestamp(0x00010000, 0x0000FFFF));
    }

    #[test]
    fn LatestTimestampWrap() {
        assert_eq!(0x00000000, LatestTimestamp(0x00000000, 0xFFFFFFFF));
        assert_eq!(0x00000000, LatestTimestamp(0x00000000, 0xFFFF0000));
        assert_eq!(0x0000FFFF, LatestTimestamp(0x0000FFFF, 0xFFFFFFFF));
        assert_eq!(0x0000FFFF, LatestTimestamp(0x0000FFFF, 0xFFFF0000));

        assert_eq!(0x00000000, LatestTimestamp(0xFFFFFFFF, 0x00000000));
        assert_eq!(0x00000000, LatestTimestamp(0xFFFF0000, 0x00000000));
        assert_eq!(0x0000FFFF, LatestTimestamp(0xFFFFFFFF, 0x0000FFFF));
        assert_eq!(0x0000FFFF, LatestTimestamp(0xFFFF0000, 0x0000FFFF));
    }
}
