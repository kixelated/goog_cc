use std::sync::LazyLock;
use std::time::Instant;

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

pub fn TimeMillis() -> i64 {
    START.elapsed().as_millis() as i64
}
