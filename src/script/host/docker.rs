use super::HostContext;
use super::http::NativeResult;
use crate::view::native_error;
use rhai::{Module, Shared};

pub(crate) fn module(context: HostContext) -> Shared<Module> {
    let mut module = Module::new();
    let containers = context.containers;
    module.set_native_fn("containers", move || -> NativeResult {
        rhai::serde::to_dynamic(&containers).map_err(|err| native_error(err.to_string()))
    });
    module.into()
}
