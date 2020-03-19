# rdp_websocket_client is a html5 RDP client built in rust and binded in webassemly using wasm-bindgen

## Getting started : 

### Install wasm pack

[wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) is tool for assembling and packaging Rust crates that target WebAssembly.

### Install npm 

```sh
https://nodejs.org/en/
```

## Start project

```sh
$ cd js_wrapper
$ npm install
```

### How to run in debug mode

Build and run the project.

```sh
$ cd js_wrapper
$ npm start
```

You can then open [localhost:8080](http://localhost:8080).
This command also auto-reloads when the project changes.

### How to build in release mode

Builds the project and places it into the `dist` folder.

```sh
$ cd js_wrapper
$ npm run build
```

### How to run unit tests

```sh
# Runs tests in Firefox
npm test -- --firefox

# Runs tests in Chrome
npm test -- --chrome

# Runs tests in Safari
npm test -- --safari
```

## What does each file do?

* `js_wrapper/package.json` contains the standard npm metadata. You put your JavaScript dependencies in here. You must change this file with your details (author, name, version)

* `js_wrapper/webpack.config.js` contains the Webpack configuration. You shouldn't need to change this, unless you have very special needs.

* the `js_wrapper/entry.js` is the entry point of the project it imports `index.js`.

* the `js_wrapper/index.js` implements a JavaScript web client example.

* The `js_wrapper/rdp-wasm` folder contains the generated JavaScript code used as an api to build the WaykNow client.

* The `js_wrapper/static` folder contains any files that you want copied as-is into the final build. It contains an `index.html` file which loads the `index.js` file.

* The `rust` folder contains the rust implementation of the WaykNow client
