use rhai::{Module, Shared};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn module() -> Shared<Module> {
    let mut module = Module::new();
    module.set_native_fn("now", || -> Result<i64, Box<rhai::EvalAltResult>> {
        let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
            Err(_) => 0,
        };
        Ok(secs)
    });
    module.into()
}
