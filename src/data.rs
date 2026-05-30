use chrono::{DateTime, TimeZone, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone, Debug)]
pub struct OrderBookLevel {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Clone, Debug)]
pub struct OrderBook {
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

#[derive(Clone, Debug)]
pub struct Position {
    pub symbol: String,
    pub side: Side,
    pub entry_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub unrealized_pnl: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Side {
    Long,
    Short,
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub quantity: f64,
    pub pnl: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub quantity: f64,
    pub filled: f64,
    pub status: OrderStatus,
}

impl Order {
    pub fn fill_pct(&self) -> f64 {
        if self.quantity == 0.0 { 0.0 } else { self.filled / self.quantity * 100.0 }
    }
    pub fn remaining(&self) -> f64 {
        (self.quantity - self.filled).max(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct MarketData {
    pub symbol: String,
    pub last_price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume_24h: f64,
    pub change_24h_pct: f64,
    pub high_24h: f64,
    pub low_24h: f64,
}

/// One bucket in the volume profile: (mid_price, total_volume)
#[derive(Clone, Debug)]
pub struct VolBucket {
    pub price: f64,
    pub volume: f64,
}

// ─── Yahoo Finance fetch ──────────────────────────────────────────────────────

pub struct YahooData {
    pub candles: Vec<Candle>,
    pub market: MarketData,
}

pub fn fetch_yahoo(symbol: &str) -> Result<YahooData, String> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=5m&range=5d&includePrePost=false",
        symbol
    );

    let resp = ureq::get(&url)
        .set(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("JSON parse error: {e}"))?;

    if let Some(err) = json["chart"]["error"].as_object() {
        return Err(format!("Yahoo error: {:?}", err));
    }

    let result = json["chart"]["result"]
        .get(0)
        .ok_or("empty result")?;

    let meta = &result["meta"];
    let timestamps = result["timestamp"]
        .as_array()
        .ok_or("missing timestamps")?;

    let q = &result["indicators"]["quote"][0];
    let opens   = q["open"  ].as_array().ok_or("missing open"  )?;
    let highs   = q["high"  ].as_array().ok_or("missing high"  )?;
    let lows    = q["low"   ].as_array().ok_or("missing low"   )?;
    let closes  = q["close" ].as_array().ok_or("missing close" )?;
    let volumes = q["volume"].as_array().ok_or("missing volume")?;

    let candles: Vec<Candle> = timestamps
        .iter()
        .enumerate()
        .filter_map(|(i, ts)| {
            let secs = ts.as_i64()?;
            let dt = Utc.timestamp_opt(secs, 0).single()?;
            let open   = opens  .get(i)?.as_f64()?;
            let high   = highs  .get(i)?.as_f64()?;
            let low    = lows   .get(i)?.as_f64()?;
            let close  = closes .get(i)?.as_f64()?;
            let volume = volumes.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0);
            Some(Candle { timestamp: dt, open, high, low, close, volume })
        })
        .collect();

    if candles.is_empty() {
        return Err("no candles returned".into());
    }

    let last_price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
    let spread = last_price * 0.0001;
    let bid  = meta["bid" ].as_f64().unwrap_or(last_price - spread);
    let ask  = meta["ask" ].as_f64().unwrap_or(last_price + spread);
    let high_24h = meta["regularMarketDayHigh"  ].as_f64().unwrap_or(last_price);
    let low_24h  = meta["regularMarketDayLow"   ].as_f64().unwrap_or(last_price);
    let vol_24h  = meta["regularMarketVolume"   ].as_f64().unwrap_or(0.0);
    let prev_close = meta["chartPreviousClose"  ].as_f64().unwrap_or(last_price);
    let change_pct = if prev_close != 0.0 {
        (last_price - prev_close) / prev_close * 100.0
    } else { 0.0 };

    Ok(YahooData {
        candles,
        market: MarketData {
            symbol: symbol.to_uppercase(),
            last_price,
            bid,
            ask,
            volume_24h: vol_24h,
            change_24h_pct: change_pct,
            high_24h,
            low_24h,
        },
    })
}

// ─── Volume profile ───────────────────────────────────────────────────────────

/// Aggregate candle volume into `n_buckets` price levels.
/// Returns buckets ordered low → high.
pub fn volume_profile(candles: &[Candle], n_buckets: usize) -> Vec<VolBucket> {
    if candles.is_empty() || n_buckets == 0 {
        return vec![];
    }

    let min_p = candles.iter().map(|c| c.low ).fold(f64::INFINITY,     f64::min);
    let max_p = candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
    let range = max_p - min_p;
    if range < 1e-9 {
        return vec![];
    }

    let bucket_size = range / n_buckets as f64;
    let mut vols = vec![0.0_f64; n_buckets];

    for c in candles {
        let lo_b = ((c.low  - min_p) / bucket_size).floor() as usize;
        let hi_b = ((c.high - min_p) / bucket_size).floor() as usize;
        let lo_b = lo_b.min(n_buckets - 1);
        let hi_b = hi_b.min(n_buckets - 1);
        let n = (hi_b - lo_b + 1) as f64;
        for b in lo_b..=hi_b {
            vols[b] += c.volume / n;
        }
    }

    vols.iter()
        .enumerate()
        .map(|(i, &v)| VolBucket {
            price: min_p + (i as f64 + 0.5) * bucket_size,
            volume: v,
        })
        .collect()
}

// ─── Order book simulation ────────────────────────────────────────────────────

pub fn generate_order_book(mid: f64, levels: usize) -> OrderBook {
    let mut rng = rand::thread_rng();
    let tick = mid * 0.00005; // 0.5 bp tick

    let bids = (0..levels)
        .map(|i| OrderBookLevel {
            price: mid - tick * (i + 1) as f64,
            quantity: rng.gen_range(0.1..20.0),
        })
        .collect();

    let asks = (0..levels)
        .map(|i| OrderBookLevel {
            price: mid + tick * (i + 1) as f64,
            quantity: rng.gen_range(0.1..20.0),
        })
        .collect();

    OrderBook { bids, asks }
}

// Light micro-fluctuation between real fetches
pub fn simulate_tick(data: &mut MarketData, rng: &mut impl Rng) {
    let pct = rng.gen_range(-0.0002..0.0002);
    data.last_price *= 1.0 + pct;
    let spread = data.last_price * 0.0001;
    data.bid = data.last_price - rng.gen_range(spread * 0.5..spread * 1.5);
    data.ask = data.last_price + rng.gen_range(spread * 0.5..spread * 1.5);
    data.high_24h = data.high_24h.max(data.last_price);
    data.low_24h  = data.low_24h .min(data.last_price);
    data.volume_24h += rng.gen_range(0.0..500.0);
}
