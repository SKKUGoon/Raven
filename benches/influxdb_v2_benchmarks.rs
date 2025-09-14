// Benchmarks for InfluxDB v2 client performance

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use market_data_subscription_server::database::influx_client::{
    create_candle_datapoint, create_funding_rate_datapoint, create_orderbook_datapoint,
    create_trade_datapoint, create_wallet_update_datapoint, InfluxClient, InfluxConfig,
};
use market_data_subscription_server::types::{
    CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn create_test_orderbook_snapshot() -> OrderBookSnapshot {
    OrderBookSnapshot {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        best_bid_price: 45000.0,
        best_bid_quantity: 1.5,
        best_ask_price: 45001.0,
        best_ask_quantity: 1.2,
        sequence: 12345,
    }
}

fn create_test_trade_snapshot() -> TradeSnapshot {
    TradeSnapshot {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        price: 45000.5,
        quantity: 0.1,
        side: "buy".to_string(),
        trade_id: 123456,
    }
}

fn create_test_candle_data() -> CandleData {
    CandleData {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        open: 45000.0,
        high: 45100.0,
        low: 44900.0,
        close: 45050.0,
        volume: 150.5,
        interval: "1m".to_string(),
        exchange: "binance".to_string(),
    }
}

fn create_test_funding_rate() -> FundingRateData {
    FundingRateData {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        rate: 0.0001,
        next_funding_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 28800000,
        exchange: "binance".to_string(),
    }
}

fn benchmark_datapoint_creation(c: &mut Criterion) {
    let orderbook_snapshot = create_test_orderbook_snapshot();
    let trade_snapshot = create_test_trade_snapshot();
    let candle_data = create_test_candle_data();
    let funding_rate = create_test_funding_rate();

    c.bench_function("create_orderbook_datapoint", |b| {
        b.iter(|| create_orderbook_datapoint(black_box(&orderbook_snapshot)))
    });

    c.bench_function("create_trade_datapoint", |b| {
        b.iter(|| create_trade_datapoint(black_box(&trade_snapshot)))
    });

    c.bench_function("create_candle_datapoint", |b| {
        b.iter(|| create_candle_datapoint(black_box(&candle_data)))
    });

    c.bench_function("create_funding_rate_datapoint", |b| {
        b.iter(|| create_funding_rate_datapoint(black_box(&funding_rate)))
    });

    c.bench_function("create_wallet_update_datapoint", |b| {
        b.iter(|| {
            create_wallet_update_datapoint(
                black_box("user123"),
                black_box("BTC"),
                black_box(1.5),
                black_box(0.1),
                black_box(1640995200000),
            )
        })
    });
}

fn benchmark_batch_datapoint_creation(c: &mut Criterion) {
    c.bench_function("create_100_orderbook_datapoints", |b| {
        b.iter(|| {
            let mut datapoints = Vec::new();
            for i in 0..100 {
                let mut snapshot = create_test_orderbook_snapshot();
                snapshot.sequence = i;
                snapshot.timestamp += i as i64;
                datapoints.push(create_orderbook_datapoint(&snapshot).unwrap());
            }
            black_box(datapoints)
        })
    });

    c.bench_function("create_1000_mixed_datapoints", |b| {
        b.iter(|| {
            let mut datapoints = Vec::new();
            for i in 0..1000 {
                match i % 4 {
                    0 => {
                        let mut snapshot = create_test_orderbook_snapshot();
                        snapshot.sequence = i;
                        datapoints.push(create_orderbook_datapoint(&snapshot).unwrap());
                    }
                    1 => {
                        let mut trade = create_test_trade_snapshot();
                        trade.trade_id = i;
                        datapoints.push(create_trade_datapoint(&trade).unwrap());
                    }
                    2 => {
                        let mut candle = create_test_candle_data();
                        candle.timestamp += i as i64;
                        datapoints.push(create_candle_datapoint(&candle).unwrap());
                    }
                    3 => {
                        let mut funding = create_test_funding_rate();
                        funding.timestamp += i as i64;
                        datapoints.push(create_funding_rate_datapoint(&funding).unwrap());
                    }
                    _ => unreachable!(),
                }
            }
            black_box(datapoints)
        })
    });
}

fn benchmark_client_operations(c: &mut Criterion) {
    c.bench_function("influx_client_creation", |b| {
        b.iter(|| {
            let config = InfluxConfig::default();
            black_box(InfluxClient::new(config))
        })
    });

    c.bench_function("get_pool_status", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = InfluxConfig::default();
        let client = InfluxClient::new(config);

        b.iter(|| rt.block_on(async { black_box(client.get_pool_status().await) }))
    });

    c.bench_function("process_empty_dead_letter_queue", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = InfluxConfig::default();
        let client = InfluxClient::new(config);

        b.iter(|| rt.block_on(async { black_box(client.process_dead_letter_queue().await) }))
    });
}

fn benchmark_circuit_breaker(c: &mut Criterion) {
    use market_data_subscription_server::database::influx_client::{
        CircuitBreaker, CircuitBreakerConfig,
    };

    c.bench_function("circuit_breaker_can_execute", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = CircuitBreakerConfig::default();
        let breaker = CircuitBreaker::new(config);

        b.iter(|| rt.block_on(async { black_box(breaker.can_execute().await) }))
    });

    c.bench_function("circuit_breaker_record_success", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = CircuitBreakerConfig::default();
        let breaker = CircuitBreaker::new(config);

        b.iter(|| rt.block_on(async { breaker.record_success().await }))
    });

    c.bench_function("circuit_breaker_get_state", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = CircuitBreakerConfig::default();
        let breaker = CircuitBreaker::new(config);

        b.iter(|| rt.block_on(async { black_box(breaker.get_state().await) }))
    });
}

criterion_group!(
    benches,
    benchmark_datapoint_creation,
    benchmark_batch_datapoint_creation,
    benchmark_client_operations,
    benchmark_circuit_breaker
);
criterion_main!(benches);
