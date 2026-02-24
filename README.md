Web build
```
cargo build --release --target wasm32-unknown-unknown && wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/my_game.wasm
```