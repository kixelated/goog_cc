use lazy_static::lazy_static;
use std::time::Instant;

lazy_static! {
    static ref START: Instant = Instant::now();
}

pub fn TimeMillis() -> i64 {
    START.elapsed().as_millis() as i64
}
