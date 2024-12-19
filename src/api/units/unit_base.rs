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
                Self(((self.0 + resolution.0 / 2) / resolution.0) * resolution.0)
            }

            pub const fn RoundUpTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self(((self.0 + resolution.0 - 1) / resolution.0) * resolution.0)
            }

            pub const fn RoundDownTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self((self.0 / resolution.0) * resolution.0)
            }

            pub const fn FromFraction<T: Into<Self>>(denominator: i64, value: T) -> Self {
                assert!(denominator >= 0);
                Self((value.into().0 * denominator))
            }

            pub const fn ToFraction<T: From<Self>> (&self, denominator: i64) -> T{
                self.ToFractionOr(denominator, T::from(self.0))
            }

            pub const fn ToFractionOr<T: From<Self>>(&self, denominator: i64, fallback_value: T) -> T {
                assert!(denominator >= 0);
                if self.IsFinite() {
                    T::from(self.0 / denominator)
                } else {
                    fallback_value
                }
            }

            pub const fn ToMultiple<T: From<Self>>(&self, factor: i64) -> T {
                assert!(factor >= 0);
                T::from(self.0 * factor)
            }
        }

        impl From<$ty> for f64{
            fn from(value: $ty) -> Self {
                if value.IsPlusInfinity() {
                    return f64::INFINITY;
                } else if value.IsMinusInfinity() {
                    return f64::NEG_INFINITY;
                }
                value.0 as f64
            }
        }

        impl From<f64> for $ty {
            fn from(value: f64) -> Self {
                assert!(value >= 0.0);
                if value == f64::INFINITY {
                    return Self::PlusInfinity();
                } else if value == f64::NEG_INFINITY {
                    return Self::MinusInfinity();
                }
                Self(value as i64)
            }
        }

        impl From<i64> for $ty {
            fn from(value: i64) -> Self {
                Self(value)
            }
        }

        impl From<i32> for $ty {
            fn from(value: i32) -> Self {
                assert!(value >= 0);
                Self(value as i64)
            }
        }

        impl From<isize> for $ty {
            fn from(value: isize) -> Self {
                assert!(value >= 0);
                Self(value as i64)
            }
        }

        impl TryFrom<$ty> for i64 {
            type Error = ();

            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                if value.IsFinite() {
                    Ok(value.0)
                } else {
                    Err(())
                }
            }
        }
     };
 }

pub(crate) use unit_base;


    #[cfg(test)]
    mod test {

       use super::*;

unit_base!(TestUnit);

impl TestUnit {
  pub const fn FromKilo<T: Into<Self>>(kilo: T) -> Self {
    return Self::FromFraction(1000, kilo);
  }
  pub const fn ToKilo<T: From<Self>>(&self) -> T {
    return self.ToFraction(1000);
  }

  pub const fn ToKiloOr<T: From<Self>>(&self, fallback: T) -> T {
    return self.ToFractionOr(1000, fallback);
  }

  pub const fn ToMilli<T: From<Self>>(&self) -> T {
    return self.ToMultiple(1000);
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
  assert!(TestUnit::from(0.0).IsZero());
  assert!(TestUnit::from(f64::INFINITY).IsPlusInfinity());
  assert!(TestUnit::from(-f64::INFINITY).IsMinusInfinity());
  assert!(TestUnit::from(250.0) == TestUnit::from(250));
  assert!(TestUnit::from(-250.0) == TestUnit::from(-250));

  assert!(kTestUnitPlusInf > kTestUnitZero);

  const kTestUnitKilo: TestUnit = TestUnit::FromKilo(kValue);
  const kTestUnitValue: TestUnit = TestUnit::from(kValue);

  assert!(kTestUnitKilo.ToKiloOr(0) == kValue);
  assert!(kTestUnitValue.try_into().unwrap_or(0) == kValue);
  assert!(TestUnit::from(500) / 2 == TestUnit::from(250));
  assert!(TestUnit::from(500.0) / 2 == TestUnit::from(250.0));
}

#[test]
fn GetBackSameValues() {
  const kValue: i64 = 499;
  for sign in [-1, 0, 1] {
    let value: i64 = kValue * sign;
    assert_eq!(TestUnit::FromKilo(value).ToKilo(), value);
    assert_eq!(TestUnit::from(value).into::<i64>(), value);
  }
  assert_eq!(TestUnit::Zero().into::<i64>(), 0);
}

#[test]
fn GetDifferentPrefix() {
  const kValue: i64 = 3000000;
  assert_eq!(TestUnit::from(kValue).ToKilo(), kValue / 1000);
  assert_eq!(TestUnit::from(kValue).into::<i64>(), kValue * 1000);
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
  const kMaxInt: isize = isize::MAX;
  assert_eq!(TestUnit::FromKilo(kMaxInt).into::<i64>(),
            (kMaxInt as i64) * 1000);
}

#[test]
fn ConvertsToAndFromDouble() {
  const kValue: i64 = 17017;
  const kMilliDouble: f64 = kValue as f64 * 1e3;
  const kValueDouble: f64 = kValue as f64;
  const kKiloDouble: f64 = kValue as f64 * 1e-3;

  assert_eq!(TestUnit::from(kValue).ToKilo::<f64>(), kKiloDouble);
  assert_eq!(TestUnit::FromKilo(kKiloDouble).into::<f64>(), kValue);

  assert_eq!(TestUnit::from(kValue).into::<f64>(), kValueDouble);
  assert_eq!(TestUnit::from(kValueDouble).into::<i64>(), kValue);

  assert!((TestUnit::from(kValue).ToMilli::<f64>() - kMilliDouble).abs() <= 1.0);

  const kPlusInfinity: f64 = f64::INFINITY;
  const kMinusInfinity: f64 = -kPlusInfinity;

  assert_eq!(TestUnit::PlusInfinity().ToKilo::<f64>(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToKilo::<f64>(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().into::<f64>(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().into::<f64>(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().ToMilli::<f64>(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToMilli::<f64>(), kMinusInfinity);

  assert!(TestUnit::FromKilo(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::FromKilo(kMinusInfinity).IsMinusInfinity());
  assert!(TestUnit::from(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::from(kMinusInfinity).IsMinusInfinity());
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan1() {
  TestUnit::from(f64::NAN);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan2() {
  TestUnit::from(0.0 / 0.0);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan3() {
  TestUnit::from(f64::INFINITY - f64::INFINITY);
}

    }
