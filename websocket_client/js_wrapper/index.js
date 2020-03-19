const js = import("./rdp-wasm/rdp_websocket_client.js");

js.then(js => {
    js.set_log();
    let connect_button = document.getElementById("testBtn");


    connect_button.onclick = function(){
        let ip = document.getElementById("ip").value;
        let port = document.getElementById("port").value;
        let username = document.getElementById("uname").value;
        let password = document.getElementById("pswrd").value;

        js.init(ip, port);
    };

});








