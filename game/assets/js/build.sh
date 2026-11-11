#!/bin/bash
# npm install -d

node build.mjs
cp hector/hector.wasm dist/
# The wasm binary is fetched relative to the page base URL in hector/init.js
