const js = import("./rdp-wasm/rdp_websocket_client.js");

js.then(js => {
    js.set_log();
    let connect_button = document.getElementById("testBtn");
    connect_button.onclick = function(){
        js.init();
    };

});








