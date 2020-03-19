const path = require("path");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const webpack = require('webpack');
const HtmlWebpackPlugin = require('html-webpack-plugin');

const dist = path.resolve(__dirname, "dist");

module.exports = {
    mode: "production",
    entry: {
        index: "./entry.js"
    },
    output: {
        path: dist,
        filename: "./entry.js",
    },
    plugins: [
        new HtmlWebpackPlugin({template: './static/index.html'}),
        /*new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, ".")
        }),*/
        new webpack.ProvidePlugin({
            TextDecoder: ['text-encoding', 'TextDecoder'],
            TextEncoder: ['text-encoding', 'TextEncoder']
        })
    ]
};
