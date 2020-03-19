use js_sys::{ArrayBuffer, DataView};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

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
pub fn init(ip_address: String, port: u16) -> Result<(), JsValue> {
    create_session(ip_address, port)
}

/*WebSocket transport*/
pub fn create_session(ip_address: String, port: u16) -> Result<()/*todo: return webclient handle*/, JsValue> {
    /*this creates a simple websocket the work should be done before to modify the header as described
     here https://devolutions.atlassian.net/browse/WAYK-1919 */
    let ws = WebSocket::new(&format!("ws://{}:{}/", ip_address, port.to_string())).unwrap();

    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let onerror_callback = Closure::wrap(on_error_callaback(ws.clone()));
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();

    let onopen_callback = Closure::wrap(on_open_callaback(ws.clone()));
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    let onclose_callback = Closure::once(on_close_callaback());
    ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
    onclose_callback.forget();

    let onmessage_callback = Closure::wrap(on_message_callback(ws.clone()));
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    Ok(())
}

pub fn on_message_callback(ws: WebSocket) -> Box<dyn FnMut(MessageEvent) -> Result<(), JsValue>> {
    Box::new(move |e: MessageEvent| -> Result<(), JsValue> {
        /*converts a js array buffer into a vec<u8>*/
        let js_buf = ArrayBuffer::from(e.data());
        let js_buf_len = js_buf.byte_length() as usize;
        let data_view = DataView::new(&js_buf, 0, js_buf_len);
        let mut data_stream = Vec::new();
        for i in 0..js_buf_len {
            data_stream.push([data_view.get_uint8(i)]);
        }


        Ok(())
    }) as Box<dyn FnMut(MessageEvent) -> Result<(), JsValue>>
}

pub fn on_open_callaback(
    ws: WebSocket,
) -> Box<dyn FnMut(JsValue) -> Result<(), JsValue>> {
    Box::new(move |_| -> Result<(), JsValue> {
        console_log!("websocket opened!");
        Ok(())
    }) as Box<dyn FnMut(JsValue) -> Result<(), JsValue>>
}

pub fn on_error_callaback(ws: WebSocket) -> Box<dyn FnMut(ErrorEvent) -> Result<(), JsValue>> {
    Box::new(move |e: ErrorEvent| {
        ws.close_with_code_and_reason(1006, &e.message())?;
        Ok(())
    }) as Box<dyn FnMut(ErrorEvent) -> Result<(), JsValue>>
}

pub fn on_close_callaback() -> Box<dyn FnOnce(web_sys::CloseEvent) -> Result<(), JsValue>> {
    Box::new(move |_| -> Result<(), JsValue> {
        log::info!("Connection with host closed");
        Ok(())
    }) as Box<dyn FnOnce(web_sys::CloseEvent) -> Result<(), JsValue>>
}

pub fn send_message(buffer: &mut [u8], ws: WebSocket) -> Result<(), JsValue> {
    ws.send_with_u8_array(buffer)
}
