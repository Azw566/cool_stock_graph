# Trading TUI — Architecture & Linking Guide

## Overview

This project is a terminal trading dashboard built with **Ratatui** (a Rust TUI framework).
It renders live-updating market data, order books, positions, and trade history entirely
in the terminal — no browser or GPU required.

## Adding a Real Data Feed

The `App` struct in `app.rs` is the only place you need to change.

1. **Replace `simulate_tick`** in `data.rs` with a call to your exchange WebSocket or REST API.
   A good pattern is an `mpsc` channel fed by a background Tokio task:

   ```rust
   // In main.rs, before the render loop:
   let (tx, rx) = std::sync::mpsc::channel::<MarketData>();
   std::thread::spawn(move || { /* fetch and tx.send(…) */ });

   // In App:
   pub rx: std::sync::mpsc::Receiver<MarketData>,
   // In on_tick():
   if let Ok(md) = self.rx.try_recv() { self.market = md; }
   ```

2. **Replace `generate_order_book`** with a parsed Level-2 snapshot from your feed.

3. **Replace the static `trade_log`** with real fill events pushed from your order manager.

## Adding a New Tab

1. Add a label to the `TABS` constant in `app.rs`.
2. Add any new state fields to `App`.
3. Add a `draw_<name>_tab(f, app, area)` function in `ui.rs`.
4. Match on the new index in `draw_body()` in `ui.rs`.

That's the entire extension surface — no registration, no trait objects required.

## Running

```bash
cargo run           
cargo run --release  
```

Press **q** to quit cleanly (raw-mode terminal is always restored).
