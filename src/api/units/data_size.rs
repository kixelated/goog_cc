/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::fmt;

super::relative_unit!(DataSize);

impl DataSize {
    const ONE_SIDED: bool = true;

    pub const fn Bytes(value: i64) -> Self {
        Self::FromValue(value)
    }

    pub const fn BytesFloat(value: f64) -> Self {
        Self::FromValueFloat(value)
    }

    pub const fn Infinity() -> Self {
        Self::PlusInfinity()
    }

    pub const fn bytes(&self) -> i64 {
        self.ToValue()
    }

    pub const fn bytes_float(&self) -> f64 {
        self.ToValueFloat()
    }

    pub const fn bytes_or(&self, fallback_value: i64) -> i64 {
        self.ToValueOr(fallback_value)
    }

    pub const fn Microbits(&self) -> i64 {
        const MaxBeforeConversion: i64 = i64::MAX / 8000000;
        assert!(
            self.bytes() <= MaxBeforeConversion,
            "size is too large to be expressed in microbits"
        );
        self.bytes() * 8000000
    }
}

impl fmt::Debug for DataSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf bytes")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf bytes")
        } else {
            write!(f, "{} bytes", self.bytes())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ConstExpr() {
        const Value: i64 = 12345;
        const DataSizeZero: DataSize = DataSize::Zero();
        const DataSizeInf: DataSize = DataSize::Infinity();
        assert!(DataSize::default() == DataSizeZero);
        assert!(DataSizeZero.IsZero());
        assert!(DataSizeInf.IsInfinite());
        assert!(DataSizeInf.bytes_or(-1) == -1);
        assert!(DataSizeInf > DataSizeZero);

        const DataSize: DataSize = DataSize::Bytes(Value);
        assert!(DataSize.bytes_or(-1) == Value);

        assert_eq!(DataSize.bytes(), Value);
    }

    #[test]
    fn GetBackSameValues() {
        const Value: i64 = 123 * 8;
        assert_eq!(DataSize::Bytes(Value).bytes(), Value);
    }

    #[test]
    fn IdentityChecks() {
        const Value: i64 = 3000;
        assert!(DataSize::Zero().IsZero());
        assert!(!DataSize::Bytes(Value).IsZero());

        assert!(DataSize::Infinity().IsInfinite());
        assert!(!DataSize::Zero().IsInfinite());
        assert!(!DataSize::Bytes(Value).IsInfinite());

        assert!(!DataSize::Infinity().IsFinite());
        assert!(DataSize::Bytes(Value).IsFinite());
        assert!(DataSize::Zero().IsFinite());
    }

    #[test]
    fn ComparisonOperators() {
        const Small: i64 = 450;
        const Large: i64 = 451;
        const small: DataSize = DataSize::Bytes(Small);
        const large: DataSize = DataSize::Bytes(Large);

        assert_eq!(DataSize::Zero(), DataSize::Bytes(0));
        assert_eq!(DataSize::Infinity(), DataSize::Infinity());
        assert_eq!(small, small);
        assert!(small <= small);
        assert!(small >= small);
        assert!(small != large);
        assert!(small <= large);
        assert!(small < large);
        assert!(large >= small);
        assert!(large > small);
        assert!(DataSize::Zero() < small);
        assert!(DataSize::Infinity() > large);
    }

    #[test]
    fn ConvertsToAndFromDouble() {
        const Value: i64 = 128;
        const DoubleValue: f64 = Value as f64;

        assert_eq!(DataSize::Bytes(Value).bytes_float(), DoubleValue);
        assert_eq!(DataSize::BytesFloat(DoubleValue).bytes(), Value);

        const Infinity: f64 = f64::INFINITY;
        assert_eq!(DataSize::Infinity().bytes_float(), Infinity);
        assert!(DataSize::BytesFloat(Infinity).IsInfinite());
    }

    #[test]
    fn MathOperations() {
        const ValueA: i64 = 450;
        const ValueB: i64 = 267;
        const size_a: DataSize = DataSize::Bytes(ValueA);
        const size_b: DataSize = DataSize::Bytes(ValueB);
        assert_eq!((size_a + size_b).bytes(), ValueA + ValueB);
        assert_eq!((size_a - size_b).bytes(), ValueA - ValueB);

        const Int32Value: i32 = 123;
        const FloatValue: f64 = 123.0;
        assert_eq!((size_a * ValueB).bytes(), ValueA * ValueB);
        assert_eq!((size_a * Int32Value).bytes(), ValueA * Int32Value as i64);
        assert_eq!(
            (size_a * FloatValue).bytes_float(),
            ValueA as f64 * FloatValue
        );

        assert_eq!((size_a / 10).bytes(), ValueA / 10);
        assert_eq!(size_a / size_b, ValueA as f64 / ValueB as f64);

        let mut mutable_size: DataSize = DataSize::Bytes(ValueA);
        mutable_size += size_b;
        assert_eq!(mutable_size.bytes(), ValueA + ValueB);
        mutable_size -= size_a;
        assert_eq!(mutable_size.bytes(), ValueB);
    }
}
