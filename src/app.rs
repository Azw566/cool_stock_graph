use std::sync::mpsc::{self, Receiver, Sender};

use crate::data::{
    MarketData, Order, OrderBook, OrderStatus, Position, Side, Trade, VolBucket,
    YahooData, fetch_yahoo, generate_order_book, simulate_tick, volume_profile,
};
use chrono::Utc;
use rand::Rng;

pub const TABS: &[&str] = &["Chart", "Order Book", "Positions", "Trade Log", "Orders"];

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    OrderEntry,
    SymbolInput,
}

#[derive(PartialEq)]
pub enum OrderField {
    Price,
    Qty,
}

pub struct OrderInput {
    pub side: Side,
    pub price: String,
    pub qty: String,
    pub focus: OrderField,
}

#[derive(Clone, PartialEq)]
pub enum FetchStatus {
    Loading,
    Ok,
    Error(String),
}

pub struct App {
    pub tab_index: usize,
    pub symbol: String,
    pub candles: Vec<crate::data::Candle>,
    pub market: MarketData,
    pub order_book: OrderBook,
    pub positions: Vec<Position>,
    pub trade_log: Vec<Trade>,
    pub open_orders: Vec<Order>,
    pub volume_profile: Vec<VolBucket>,
    pub scroll_offset: usize,
    pub tick_count: u64,
    pub input_mode: InputMode,
    pub order_input: Option<OrderInput>,
    pub symbol_buf: String,
    pub next_order_id: u64,
    pub fetch_status: FetchStatus,
    fetch_tx: Sender<Result<YahooData, String>>,
    pub fetch_rx: Receiver<Result<YahooData, String>>,
    rng: rand::rngs::ThreadRng,
}

impl App {
    pub fn new() -> Self {
        let default_symbol = "SPY".to_string();
        let placeholder_price = 1.0_f64;

        let (fetch_tx, fetch_rx) = mpsc::channel::<Result<YahooData, String>>();

        // Kick off the initial fetch immediately
        {
            let tx = fetch_tx.clone();
            let sym = default_symbol.clone();
            std::thread::spawn(move || {
                tx.send(fetch_yahoo(&sym)).ok();
            });
        }

        let market = MarketData {
            symbol: default_symbol.clone(),
            last_price: placeholder_price,
            bid: placeholder_price,
            ask: placeholder_price,
            volume_24h: 0.0,
            change_24h_pct: 0.0,
            high_24h: placeholder_price,
            low_24h: placeholder_price,
        };

        App {
            tab_index: 0,
            symbol: default_symbol,
            candles: vec![],
            market,
            order_book: OrderBook { bids: vec![], asks: vec![] },
            positions: vec![],
            trade_log: vec![],
            open_orders: vec![],
            volume_profile: vec![],
            scroll_offset: 0,
            tick_count: 0,
            input_mode: InputMode::Normal,
            order_input: None,
            symbol_buf: String::new(),
            next_order_id: 1,
            fetch_status: FetchStatus::Loading,
            fetch_tx,
            fetch_rx,
            rng: rand::thread_rng(),
        }
    }

    pub fn on_tick(&mut self) {
        self.tick_count += 1;

        // Check for data from background fetch
        if let Ok(result) = self.fetch_rx.try_recv() {
            match result {
                Ok(data) => {
                    self.candles = data.candles;
                    self.market  = data.market;
                    self.volume_profile = volume_profile(&self.candles, 24);
                    self.order_book = generate_order_book(self.market.last_price, 10);
                    self.fetch_status = FetchStatus::Ok;
                }
                Err(e) => {
                    self.fetch_status = FetchStatus::Error(e);
                }
            }
        }

        // Micro-fluctuate between fetches (only if we have real data)
        if self.fetch_status == FetchStatus::Ok || matches!(&self.fetch_status, FetchStatus::Error(_)) {
            if self.market.last_price > 1.0 {
                simulate_tick(&mut self.market, &mut self.rng);
            }
        }

        // Refresh order book every second
        if self.tick_count % 4 == 0 && self.market.last_price > 1.0 {
            self.order_book = generate_order_book(self.market.last_price, 10);
        }

        // Refetch real data every ~5 minutes (1200 ticks at 250ms)
        if self.tick_count % 1200 == 0 {
            self.trigger_fetch(self.symbol.clone());
        }

        // Simulate order fills
        let ask = self.market.ask;
        let bid = self.market.bid;
        let mut filled_ids: Vec<u64> = vec![];

        for order in &mut self.open_orders {
            if order.status == OrderStatus::Filled || order.status == OrderStatus::Cancelled {
                continue;
            }
            let marketable = match order.side {
                Side::Long  => order.price >= ask,
                Side::Short => order.price <= bid,
            };
            if marketable && self.rng.gen_bool(0.35) {
                let fill = order.remaining() * self.rng.gen_range(0.25..0.9);
                order.filled += fill;
                if order.remaining() < order.quantity * 0.02 {
                    order.filled = order.quantity;
                    order.status = OrderStatus::Filled;
                    filled_ids.push(order.id);
                } else {
                    order.status = OrderStatus::PartiallyFilled;
                }
            }
        }

        for id in filled_ids {
            if let Some(o) = self.open_orders.iter().find(|o| o.id == id) {
                let pnl = match o.side {
                    Side::Long  => o.quantity * (self.market.last_price - o.price),
                    Side::Short => o.quantity * (o.price - self.market.last_price),
                };
                self.trade_log.insert(0, Trade {
                    timestamp: Utc::now(),
                    symbol:   o.symbol.clone(),
                    side:     o.side.clone(),
                    price:    o.price,
                    quantity: o.quantity,
                    pnl,
                });
            }
        }
        self.open_orders.retain(|o| o.status != OrderStatus::Filled);
    }

    fn trigger_fetch(&self, sym: String) {
        let tx = self.fetch_tx.clone();
        std::thread::spawn(move || {
            tx.send(fetch_yahoo(&sym)).ok();
        });
    }

    // ─── symbol input ──────────────────────────────────────────────────────

    pub fn start_symbol_input(&mut self) {
        self.symbol_buf.clear();
        self.input_mode = InputMode::SymbolInput;
    }

    pub fn cancel_symbol_input(&mut self) {
        self.symbol_buf.clear();
        self.input_mode = InputMode::Normal;
    }

    pub fn submit_symbol(&mut self) {
        let sym = self.symbol_buf.trim().to_uppercase();
        if !sym.is_empty() {
            self.symbol = sym.clone();
            self.fetch_status = FetchStatus::Loading;
            self.trigger_fetch(sym);
        }
        self.symbol_buf.clear();
        self.input_mode = InputMode::Normal;
    }

    pub fn symbol_char(&mut self, c: char) {
        if c.is_alphanumeric() || c == '-' || c == '.' {
            self.symbol_buf.push(c.to_ascii_uppercase());
        }
    }

    pub fn symbol_backspace(&mut self) {
        self.symbol_buf.pop();
    }

    // ─── order entry ───────────────────────────────────────────────────────

    pub fn start_order(&mut self, side: Side) {
        if self.market.last_price <= 1.0 { return; }
        let price = match side {
            Side::Long  => self.market.ask,
            Side::Short => self.market.bid,
        };
        self.order_input = Some(OrderInput {
            side,
            price: format!("{:.2}", price),
            qty: "1".to_string(),
            focus: OrderField::Price,
        });
        self.input_mode = InputMode::OrderEntry;
    }

    pub fn cancel_order_entry(&mut self) {
        self.order_input = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn submit_order(&mut self) {
        if let Some(input) = self.order_input.take() {
            let price: f64 = input.price.parse().unwrap_or(self.market.last_price);
            let qty:   f64 = input.qty  .parse().unwrap_or(0.0);
            if qty > 0.0 && price > 0.0 {
                self.open_orders.push(Order {
                    id: self.next_order_id,
                    timestamp: Utc::now(),
                    symbol: self.symbol.clone(),
                    side: input.side,
                    price,
                    quantity: qty,
                    filled: 0.0,
                    status: OrderStatus::Pending,
                });
                self.next_order_id += 1;
            }
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn handle_char(&mut self, c: char) {
        if let Some(input) = &mut self.order_input {
            let field = match input.focus {
                OrderField::Price => &mut input.price,
                OrderField::Qty   => &mut input.qty,
            };
            if c.is_ascii_digit() || (c == '.' && !field.contains('.')) {
                field.push(c);
            }
        }
    }

    pub fn handle_backspace(&mut self) {
        if let Some(input) = &mut self.order_input {
            match input.focus {
                OrderField::Price => { input.price.pop(); }
                OrderField::Qty   => { input.qty  .pop(); }
            }
        }
    }

    pub fn toggle_order_field(&mut self) {
        if let Some(input) = &mut self.order_input {
            input.focus = match input.focus {
                OrderField::Price => OrderField::Qty,
                OrderField::Qty   => OrderField::Price,
            };
        }
    }

    pub fn cancel_selected_order(&mut self) {
        if self.tab_index == 4 && !self.open_orders.is_empty() {
            let idx = self.scroll_offset.min(self.open_orders.len() - 1);
            self.open_orders[idx].status = OrderStatus::Cancelled;
            self.open_orders.retain(|o| o.status != OrderStatus::Cancelled);
            if self.scroll_offset > 0 && self.scroll_offset >= self.open_orders.len() {
                self.scroll_offset -= 1;
            }
        }
    }

    // ─── navigation ────────────────────────────────────────────────────────

    pub fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % TABS.len();
        self.scroll_offset = 0;
    }

    pub fn prev_tab(&mut self) {
        self.tab_index = (self.tab_index + TABS.len() - 1) % TABS.len();
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self)   { self.scroll_offset = self.scroll_offset.saturating_sub(1); }
    pub fn scroll_down(&mut self) { self.scroll_offset += 1; }
    pub fn reset_view(&mut self)  { self.scroll_offset = 0; }

    pub fn total_unrealized_pnl(&self) -> f64 {
        self.positions.iter().map(|p| p.unrealized_pnl).sum()
    }
    pub fn total_realized_pnl(&self) -> f64 {
        self.trade_log.iter().map(|t| t.pnl).sum()
    }
}
