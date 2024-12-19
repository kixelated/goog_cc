// UnitBase is a superclass in C++.
// The closest we can do in Rust is a macro, as traits don't support const.
 macro_rules! unit_base {
     ($ty:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
        pub struct $ty(i64);

        impl $ty {
            pub const fn Zero() -> Self {
                Self(0)
            }
            pub const fn PlusInfinity() -> Self {
                Self(i64::MAX)
            }
            pub const fn MinusInfinity() -> Self {
                Self(i64::MIN)
            }

            pub const fn IsZero(&self) -> bool {
                self.0 == 0
            }
            pub const fn IsFinite(&self) -> bool {
                !self.IsInfinite()
            }
            pub const fn IsInfinite(&self) -> bool {
                self.0 == i64::MAX || self.0 == i64::MIN
            }

            pub const fn IsPlusInfinity(&self) -> bool {
                self.0 == i64::MAX
            }

            pub const fn IsMinusInfinity(&self) -> bool {
                self.0 == i64::MIN
            }

            pub const fn RoundTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue(((self.0 + resolution.0 / 2) / resolution.0) * resolution.0)
            }

            pub const fn RoundUpTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue(((self.0 + resolution.0 - 1) / resolution.0) * resolution.0)
            }

            pub const fn RoundDownTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue((self.0 / resolution.0) * resolution.0)
            }

            pub const fn FromFraction(denominator: i64, value: i64) -> Self {
                assert!(denominator >= 0);
                Self::FromValue(value * denominator)
            }

            pub const fn FromFractionFloat(denominator: f64, value: f64) -> Self {
                Self::FromValueFloat(value * denominator)
            }

            pub const fn ToFraction(&self, denominator: i64) -> i64 {
                assert!(denominator >= 0);
                assert!(self.IsFinite());
                self.ToValue() / denominator
            }

            pub const fn ToFractionFloat(&self, denominator: f64) -> f64 {
                assert!(denominator >= 0.0);
                assert!(self.IsFinite());
                self.ToValueFloat() / denominator
            }

            pub const fn ToFractionOr(&self, denominator: i64, fallback_value: i64) -> i64 {
                assert!(denominator >= 0);
                if self.IsFinite() {
                    self.ToValue() / denominator
                } else {
                    fallback_value
                }
            }

            pub const fn ToFractionOrFloat(&self, denominator: f64, fallback_value: f64) -> f64 {
                assert!(denominator >= 0.0);
                if self.IsFinite() {
                    self.ToValueFloat() / denominator
                } else {
                    fallback_value
                }
            }

            pub const fn ToMultiple(&self, factor: i64) -> i64 {
                assert!(factor >= 0);
                self.ToValue() * factor
            }

            pub const fn ToMultipleFloat(&self, factor: f64) -> f64 {
                assert!(factor >= 0.0);
                self.ToValueFloat() * factor
            }

            pub const fn FromValue(value: i64) -> Self {
                assert!(value != i64::MAX && value != i64::MIN);
                Self(value)
            }

            pub const fn FromValueFloat(value: f64) -> Self {
                if Self::ONE_SIDED {
                    assert!(value >= 0.0);
                }

                if value == f64::INFINITY {
                    Self::PlusInfinity()
                } else if value == f64::NEG_INFINITY {
                    Self::MinusInfinity()
                } else {
                    Self(value as i64)
                }
            }

            pub const fn ToValue(&self) -> i64 {
                assert!(self.IsFinite());
                self.0
            }

            pub const fn ToValueOr(&self, fallback_value: i64) -> i64 {
                if self.IsFinite() {
                    self.0
                } else {
                    fallback_value
                }
            }

            pub const fn ToValueOrFloat(&self, fallback_value: f64) -> f64 {
                if self.IsFinite() {
                    self.0 as f64
                } else {
                    fallback_value
                }
            }

            pub const fn ToValueFloat(&self) -> f64 {
                if self.IsPlusInfinity() {
                    f64::INFINITY
                } else if self.IsMinusInfinity() {
                    f64::NEG_INFINITY
                } else {
                    self.0 as f64
                }
            }
        }

        /*
        impl From<$ty> for f64{
            fn from(value: $ty) -> Self {
                value.ToValueFloat()
            }
        }

        impl From<f64> for $ty {
            fn from(value: f64) -> Self {
                Self::FromValueFloat(value)
            }
        }

        impl From<f32> for $ty {
            fn from(value: f32) -> Self {
                Self::FromValueFloat(value as f64)
            }
        }

        impl From<i64> for $ty {
            fn from(value: i64) -> Self {
                Self::FromValue(value)
            }
        }

        impl From<i32> for $ty {
            fn from(value: i32) -> Self {
                Self::FromValue(value as i64)
            }
        }

        impl From<isize> for $ty {
            fn from(value: isize) -> Self {
                Self::FromValue(value as i64)
            }
        }

        impl From<$ty> for i64 {
            fn from(value: $ty) -> Self {
                value.ToValue()
            }
        }

        impl From<$ty> for i32 {
            fn from(value: $ty) -> Self {
                value.ToValue().try_into().unwrap()
            }
        }

        impl From<$ty> for isize {
            fn from(value: $ty) -> Self {
                value.ToValue().try_into().unwrap()
            }
        }
        */
     };
 }

pub(crate) use unit_base;


    #[cfg(test)]
    mod test {

       use std::fmt;

    use super::*;

unit_base!(TestUnit);

impl TestUnit {
    const ONE_SIDED: bool = true;

    pub const fn FromKilo(kilo: i64) -> Self {
        Self::FromFraction(1000, kilo)
    }

    pub fn FromKiloFloat(kilo: f64) -> Self {
        Self::FromFractionFloat(1000.0, kilo)
    }
  pub const fn ToKilo(&self) -> i64 {
    self.ToFraction(1000)
  }

  pub const fn ToKiloFloat(&self) -> f64 {
    self.ToFractionFloat(1000.0)
  }

  pub const fn ToKiloOr(&self, fallback: i64) -> i64 {
    self.ToFractionOr(1000, fallback)
  }

  pub const fn ToKiloFloatOr(&self, fallback: f64) -> f64 {
    self.ToFractionOrFloat(1000.0, fallback)
  }

  pub const fn ToMilli(&self) -> i64 {
    self.ToMultiple(1000)
  }

    pub const fn ToMilliFloat(&self) -> f64 {
        self.ToMultipleFloat(1000.0)
    }
}

impl fmt::Debug for TestUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

#[test]
fn ConstExpr() {
  const kValue: i64 = -12345;
  const kTestUnitZero: TestUnit = TestUnit::Zero();
  const kTestUnitPlusInf: TestUnit = TestUnit::PlusInfinity();
  const kTestUnitMinusInf: TestUnit = TestUnit::MinusInfinity();

  assert!(kTestUnitZero.IsZero());
  assert!(kTestUnitPlusInf.IsPlusInfinity());
  assert!(kTestUnitMinusInf.IsMinusInfinity());
  assert!(kTestUnitPlusInf.ToKiloOr(-1) == -1);

  // Check from is constexpr for floats.
  assert!(TestUnit::FromValueFloat(0.0).IsZero());
  assert!(TestUnit::FromValueFloat(f64::INFINITY).IsPlusInfinity());
  assert!(TestUnit::FromValueFloat(-f64::INFINITY).IsMinusInfinity());
  assert!(TestUnit::FromValueFloat(250.0) == TestUnit::FromValue(250));
  assert!(TestUnit::FromValueFloat(-250.0) == TestUnit::FromValue(-250));

  assert!(kTestUnitPlusInf > kTestUnitZero);

  const kTestUnitKilo: TestUnit = TestUnit::FromKilo(kValue);
  const kTestUnitValue: TestUnit = TestUnit(kValue);

  assert!(kTestUnitKilo.ToKiloOr(0) == kValue);
  assert!(kTestUnitValue.ToValueOr(0) == kValue);
}

#[test]
fn GetBackSameValues() {
  const kValue: i64 = 499;
  for sign in [-1, 0, 1] {
    let value: i64 = kValue * sign;
    assert_eq!(TestUnit::FromKilo(value).ToKilo(), value);
    assert_eq!(TestUnit::FromValue(value).ToValue(), value);
  }
  assert_eq!(TestUnit::Zero().ToValue(), 0);
}

#[test]
fn GetDifferentPrefix() {
  const kValue: i64 = 3000000;
  assert_eq!(TestUnit::FromValue(kValue).ToKilo(), kValue / 1000);
  assert_eq!(TestUnit::FromValue(kValue).ToValue(), kValue * 1000);
}

#[test]
fn IdentityChecks() {
  const kValue: i64 = 3000;
  assert!(TestUnit::Zero().IsZero());
  assert!(!TestUnit::FromKilo(kValue).IsZero());

  assert!(TestUnit::PlusInfinity().IsInfinite());
  assert!(TestUnit::MinusInfinity().IsInfinite());
  assert!(!TestUnit::Zero().IsInfinite());
  assert!(!TestUnit::FromKilo(-kValue).IsInfinite());
  assert!(!TestUnit::FromKilo(kValue).IsInfinite());

  assert!(!TestUnit::PlusInfinity().IsFinite());
  assert!(!TestUnit::MinusInfinity().IsFinite());
  assert!(TestUnit::FromKilo(-kValue).IsFinite());
  assert!(TestUnit::FromKilo(kValue).IsFinite());
  assert!(TestUnit::Zero().IsFinite());

  assert!(TestUnit::PlusInfinity().IsPlusInfinity());
  assert!(!TestUnit::MinusInfinity().IsPlusInfinity());

  assert!(TestUnit::MinusInfinity().IsMinusInfinity());
  assert!(!TestUnit::PlusInfinity().IsMinusInfinity());
}

#[test]
fn ComparisonOperators() {
  const kSmall: i64 = 450;
  const kLarge: i64 = 451;
  const small: TestUnit = TestUnit::FromKilo(kSmall);
  const large: TestUnit = TestUnit::FromKilo(kLarge);

  assert_eq!(TestUnit::Zero(), TestUnit::FromKilo(0));
  assert_eq!(TestUnit::PlusInfinity(), TestUnit::PlusInfinity());
  assert_eq!(small, TestUnit::FromKilo(kSmall));
  assert!(small <= TestUnit::FromKilo(kSmall));
  assert!(small >= TestUnit::FromKilo(kSmall));
  assert!(small != TestUnit::FromKilo(kLarge));
  assert!(small <= TestUnit::FromKilo(kLarge));
  assert!(small < TestUnit::FromKilo(kLarge));
  assert!(large >= TestUnit::FromKilo(kSmall));
  assert!(large > TestUnit::FromKilo(kSmall));
  assert!(TestUnit::Zero() < small);
  assert!(TestUnit::Zero() > TestUnit::FromKilo(-kSmall));
  assert!(TestUnit::Zero() > TestUnit::FromKilo(-kSmall));

  assert!(TestUnit::PlusInfinity() > large);
  assert!(TestUnit::MinusInfinity() < TestUnit::Zero());
}

#[test]
fn CanBeInititializedFromLargeInt() {
  const kMaxInt: i32 = i32::MAX;
  assert_eq!(TestUnit::FromKilo(kMaxInt as i64).ToValue(),
            (kMaxInt as i64) * 1000);
}

#[test]
fn ConvertsToAndFromDouble() {
  const kValue: i64 = 17017;
  const kMilliDouble: f64 = kValue as f64 * 1e3;
  const kValueDouble: f64 = kValue as f64;
  const kKiloDouble: f64 = kValue as f64 * 1e-3;

  assert_eq!(TestUnit::FromValue(kValue).ToKiloFloat(), kKiloDouble);
  assert_eq!(TestUnit::FromKiloFloat(kKiloDouble).ToValueFloat(), kValue as f64);

  assert_eq!(TestUnit::FromValue(kValue).ToValueFloat(), kValueDouble);
  assert_eq!(TestUnit::FromValueFloat(kValueDouble).ToValue(), kValue);

  assert!((TestUnit::FromValue(kValue).ToMilliFloat() - kMilliDouble).abs() <= 1.0);

  const kPlusInfinity: f64 = f64::INFINITY;
  const kMinusInfinity: f64 = -kPlusInfinity;

  assert_eq!(TestUnit::PlusInfinity().ToKiloFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToKiloFloat(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().ToKiloFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToKiloFloat(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().ToMilliFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToMilliFloat(), kMinusInfinity);

  assert!(TestUnit::FromKiloFloat(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::FromKiloFloat(kMinusInfinity).IsMinusInfinity());
  assert!(TestUnit::FromValueFloat(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::FromValueFloat(kMinusInfinity).IsMinusInfinity());
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan1() {
  TestUnit::FromValueFloat(f64::NAN);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan2() {
  TestUnit::FromValueFloat(0.0 / 0.0);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan3() {
  TestUnit::FromValueFloat(f64::INFINITY - f64::INFINITY);
}

    }
