/*mod app;
mod challenges;
mod colors;
mod common;
mod cutscene;
mod debug;
mod devtools;
mod edit;
mod game;
mod helpers;
mod info;
mod layer;
mod main;
mod options;
mod pregame;
mod render;
mod sandbox;*/

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm-game-of-life!");
}
