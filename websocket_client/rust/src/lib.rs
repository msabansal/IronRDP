use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn set_log() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let _ = console_log::init_with_level(log::Level::Debug).map_err(|e| {
        log(&format_args!("couldn't init logger: {}", e).to_string());
    });
}

#[wasm_bindgen]
pub fn init() -> Result<(), JsValue> {
    Ok(console_log!("HELLOW WORLD!"))
}
