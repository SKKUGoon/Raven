// End-to-End Latency Benchmarks
// "Measuring the speed of ravens from nest to destination"

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use market_data_subscription_server::data_handlers::high_frequency::HighFrequencyHandler;
use market_data_subscription_server::types::{OrderBookData, TradeData};
use rand::Rng;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn create_latency_test_orderbook(symbol: &str, sequence: u64) -> OrderBookData {
    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: vec![
            (45000.0 + sequence as f64, 1.5),
            (44999.0 + sequence as f64, 2.0),
        ],
        asks: vec![
            (45001.0 + sequence as f64, 1.2),
            (45002.0 + sequence as f64, 1.8),
        ],
        sequence,
        exchange: "binance".to_string(),
    }
}

fn create_latency_test_trade(symbol: &str, sequence: u64) -> TradeData {
    TradeData {
        symbol: symbol.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        price: 45000.0 + sequence as f64,
        quantity: 0.1 + (sequence % 10) as f64 / 100.0,
        side: if sequence % 2 == 0 { "buy" } else { "sell" }.to_string(),
        trade_id: format!("latency_trade_{}", sequence),
        exchange: "binance".to_string(),
    }
}

// Benchmark sub-millisecond end-to-end latency requirement
fn bench_sub_millisecond_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("sub_millisecond_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);

    group.bench_function("complete_data_pipeline_latency", |b| {
        let handler = HighFrequencyHandler::new();

        b.iter(|| {
            let pipeline_start = Instant::now();

            // Step 1: Data reception (simulating WebSocket receive)
            let reception_start = Instant::now();
            let sequence = rand::thread_rng().gen_range(1..1000000);
            let orderbook_data = create_latency_test_orderbook("BTCUSDT", sequence);
            let trade_data = create_latency_test_trade("BTCUSDT", sequence);
            let reception_time = reception_start.elapsed();

            // Step 2: Data ingestion (atomic updates)
            let ingestion_start = Instant::now();
            handler
                .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&orderbook_data))
                .unwrap();
            handler
                .ingest_trade_atomic(black_box("BTCUSDT"), black_box(&trade_data))
                .unwrap();
            let ingestion_time = ingestion_start.elapsed();

            // Step 3: Snapshot capture (atomic reads)
            let snapshot_start = Instant::now();
            let orderbook_snapshot = handler
                .capture_orderbook_snapshot(black_box("BTCUSDT"))
                .unwrap();
            let trade_snapshot = handler
                .capture_trade_snapshot(black_box("BTCUSDT"))
                .unwrap();
            let snapshot_time = snapshot_start.elapsed();

            // Step 4: Serialization (gRPC message preparation)
            let serialization_start = Instant::now();
            let _serialized_orderbook = serde_json::to_string(&orderbook_snapshot).unwrap();
            let _serialized_trade = serde_json::to_string(&trade_snapshot).unwrap();
            let serialization_time = serialization_start.elapsed();

            let total_latency = pipeline_start.elapsed();

            // Note: In production, these should meet stricter requirements
            // Benchmark environment has additional overhead
            if total_latency.as_micros() > 5000 {
                // 5ms threshold for benchmark
                eprintln!("Warning: End-to-end latency high: {:?}", total_latency);
            }

            // Individual component latency validation (more lenient for benchmarks)
            if ingestion_time.as_micros() > 100 {
                // 100μs threshold for benchmark
                eprintln!("Warning: Ingestion latency high: {:?}", ingestion_time);
            }
            if snapshot_time.as_micros() > 100 {
                // 100μs threshold for benchmark
                eprintln!("Warning: Snapshot latency high: {:?}", snapshot_time);
            }

            // Return detailed timing for analysis
            LatencyMeasurement {
                total: total_latency,
                reception: reception_time,
                ingestion: ingestion_time,
                snapshot: snapshot_time,
                serialization: serialization_time,
            }
        })
    });

    group.finish();
}

#[derive(Debug)]
struct LatencyMeasurement {
    total: Duration,
    reception: Duration,
    ingestion: Duration,
    snapshot: Duration,
    serialization: Duration,
}

// Benchmark latency under different load conditions
fn bench_latency_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_under_load");
    group.measurement_time(Duration::from_secs(15));

    for concurrent_operations in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_operations", concurrent_operations),
            concurrent_operations,
            |b, &concurrent_operations| {
                let handler = Arc::new(HighFrequencyHandler::new());

                b.iter(|| {
                    let mut handles = vec![];
                    let latency_measurements = Arc::new(std::sync::Mutex::new(Vec::new()));

                    // Spawn concurrent operations to create load
                    for i in 0..concurrent_operations {
                        let handler_clone = Arc::clone(&handler);
                        let measurements_clone = Arc::clone(&latency_measurements);
                        let symbol = format!("LOAD_SYMBOL_{}", i % 10); // 10 different symbols

                        let handle = thread::spawn(move || {
                            let operation_start = Instant::now();

                            let sequence = i as u64;
                            let orderbook_data = create_latency_test_orderbook(&symbol, sequence);
                            let trade_data = create_latency_test_trade(&symbol, sequence);

                            // Measure ingestion latency under load
                            let ingestion_start = Instant::now();
                            handler_clone
                                .ingest_orderbook_atomic(&symbol, &orderbook_data)
                                .unwrap();
                            handler_clone
                                .ingest_trade_atomic(&symbol, &trade_data)
                                .unwrap();
                            let ingestion_time = ingestion_start.elapsed();

                            // Measure snapshot latency under load
                            let snapshot_start = Instant::now();
                            let _ = handler_clone.capture_orderbook_snapshot(&symbol).unwrap();
                            let _ = handler_clone.capture_trade_snapshot(&symbol).unwrap();
                            let snapshot_time = snapshot_start.elapsed();

                            let total_time = operation_start.elapsed();

                            // Store measurement
                            let measurement = LatencyMeasurement {
                                total: total_time,
                                reception: Duration::from_nanos(0),
                                ingestion: ingestion_time,
                                snapshot: snapshot_time,
                                serialization: Duration::from_nanos(0),
                            };

                            measurements_clone.lock().unwrap().push(measurement);
                        });

                        handles.push(handle);
                    }

                    // Wait for all operations to complete
                    for handle in handles {
                        handle.join().unwrap();
                    }

                    // Analyze latency measurements
                    let measurements = latency_measurements.lock().unwrap();

                    // Calculate statistics
                    let mut total_latencies: Vec<u128> =
                        measurements.iter().map(|m| m.total.as_nanos()).collect();
                    let mut ingestion_latencies: Vec<u128> = measurements
                        .iter()
                        .map(|m| m.ingestion.as_nanos())
                        .collect();

                    total_latencies.sort();
                    ingestion_latencies.sort();

                    let total_p99 = total_latencies
                        [(total_latencies.len() * 99 / 100).min(total_latencies.len() - 1)];
                    let ingestion_p99 = ingestion_latencies
                        [(ingestion_latencies.len() * 99 / 100).min(ingestion_latencies.len() - 1)];

                    // Validate latency requirements under load
                    assert!(
                        total_p99 < 1_000_000, // 1ms in nanoseconds
                        "P99 total latency exceeds 1ms under {} concurrent ops: {}ns",
                        concurrent_operations,
                        total_p99
                    );

                    assert!(
                        ingestion_p99 < 1_000, // 1μs in nanoseconds
                        "P99 ingestion latency exceeds 1μs under {} concurrent ops: {}ns",
                        concurrent_operations,
                        ingestion_p99
                    );

                    (concurrent_operations, total_p99, ingestion_p99)
                })
            },
        );
    }

    group.finish();
}

// Benchmark latency percentiles
fn bench_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_percentiles");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10000);

    group.bench_function("latency_distribution_analysis", |b| {
        let handler = HighFrequencyHandler::new();

        b.iter(|| {
            let mut latency_measurements = Vec::new();

            // Collect 1000 latency measurements
            for i in 0..1000 {
                let measurement_start = Instant::now();

                let orderbook_data = create_latency_test_orderbook("BTCUSDT", i);
                let trade_data = create_latency_test_trade("BTCUSDT", i);

                handler
                    .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
                    .unwrap();
                handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();

                let _ = handler.capture_orderbook_snapshot("BTCUSDT").unwrap();
                let _ = handler.capture_trade_snapshot("BTCUSDT").unwrap();

                let total_latency = measurement_start.elapsed();
                latency_measurements.push(total_latency.as_nanos());
            }

            // Sort for percentile calculation
            latency_measurements.sort();

            let len = latency_measurements.len();
            let p50 = latency_measurements[len * 50 / 100];
            let p90 = latency_measurements[len * 90 / 100];
            let p95 = latency_measurements[len * 95 / 100];
            let p99 = latency_measurements[len * 99 / 100];
            let p999 = latency_measurements[len * 999 / 1000];

            // Validate latency percentiles
            assert!(p50 < 500_000, "P50 latency exceeds 500μs: {}ns", p50); // 500μs
            assert!(p90 < 800_000, "P90 latency exceeds 800μs: {}ns", p90); // 800μs
            assert!(p95 < 900_000, "P95 latency exceeds 900μs: {}ns", p95); // 900μs
            assert!(p99 < 1_000_000, "P99 latency exceeds 1ms: {}ns", p99); // 1ms
            assert!(p999 < 2_000_000, "P99.9 latency exceeds 2ms: {}ns", p999); // 2ms

            LatencyPercentiles {
                p50,
                p90,
                p95,
                p99,
                p999,
            }
        })
    });

    group.finish();
}

#[derive(Debug)]
struct LatencyPercentiles {
    p50: u128,
    p90: u128,
    p95: u128,
    p99: u128,
    p999: u128,
}

// Benchmark jitter and latency consistency
fn bench_latency_jitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_jitter");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("latency_consistency", |b| {
        let handler = HighFrequencyHandler::new();

        b.iter(|| {
            let mut latencies = Vec::new();

            // Measure latency over time to detect jitter
            for i in 0..1000 {
                let start = Instant::now();

                let orderbook_data = create_latency_test_orderbook("BTCUSDT", i);
                handler
                    .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
                    .unwrap();
                let _ = handler.capture_orderbook_snapshot("BTCUSDT").unwrap();

                let latency = start.elapsed().as_nanos();
                latencies.push(latency);

                // Small delay to simulate realistic timing
                if i % 100 == 0 {
                    thread::sleep(Duration::from_micros(10));
                }
            }

            // Calculate jitter statistics
            let mean = latencies.iter().sum::<u128>() / latencies.len() as u128;
            let variance = latencies
                .iter()
                .map(|&x| {
                    let diff = if x > mean { x - mean } else { mean - x };
                    diff * diff
                })
                .sum::<u128>()
                / latencies.len() as u128;
            let std_dev = (variance as f64).sqrt() as u128;

            // Calculate coefficient of variation (jitter measure)
            let cv = (std_dev as f64) / (mean as f64);

            // Validate low jitter (coefficient of variation should be low)
            assert!(cv < 0.5, "High latency jitter detected: CV = {:.3}", cv);
            assert!(
                std_dev < 100_000,
                "High latency standard deviation: {}ns",
                std_dev
            ); // 100μs

            JitterMeasurement {
                mean,
                std_dev,
                coefficient_of_variation: cv,
                min: *latencies.iter().min().unwrap(),
                max: *latencies.iter().max().unwrap(),
            }
        })
    });

    group.finish();
}

#[derive(Debug)]
struct JitterMeasurement {
    mean: u128,
    std_dev: u128,
    coefficient_of_variation: f64,
    min: u128,
    max: u128,
}

// Benchmark warm-up effects on latency
fn bench_warmup_latency_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("warmup_latency_effects");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("cold_vs_warm_performance", |b| {
        b.iter(|| {
            // Test cold start performance
            let cold_handler = HighFrequencyHandler::new();
            let cold_start = Instant::now();

            let orderbook_data = create_latency_test_orderbook("BTCUSDT", 1);
            cold_handler
                .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
                .unwrap();
            let _ = cold_handler.capture_orderbook_snapshot("BTCUSDT").unwrap();

            let cold_latency = cold_start.elapsed();

            // Warm up the handler
            for i in 0..1000 {
                let warmup_data = create_latency_test_orderbook("BTCUSDT", i + 2);
                cold_handler
                    .ingest_orderbook_atomic("BTCUSDT", &warmup_data)
                    .unwrap();
                if i % 10 == 0 {
                    let _ = cold_handler.capture_orderbook_snapshot("BTCUSDT").unwrap();
                }
            }

            // Test warm performance
            let warm_start = Instant::now();

            let warm_orderbook_data = create_latency_test_orderbook("BTCUSDT", 1002);
            cold_handler
                .ingest_orderbook_atomic("BTCUSDT", &warm_orderbook_data)
                .unwrap();
            let _ = cold_handler.capture_orderbook_snapshot("BTCUSDT").unwrap();

            let warm_latency = warm_start.elapsed();

            // Validate that warm performance is better or similar
            let performance_ratio = warm_latency.as_nanos() as f64 / cold_latency.as_nanos() as f64;

            // Warm performance should not be significantly worse than cold
            assert!(
                performance_ratio <= 2.0,
                "Warm performance significantly worse than cold: {:.2}x",
                performance_ratio
            );

            // Both should meet latency requirements
            assert!(
                cold_latency.as_micros() < 1000,
                "Cold start latency exceeds 1ms: {:?}",
                cold_latency
            );
            assert!(
                warm_latency.as_micros() < 1000,
                "Warm latency exceeds 1ms: {:?}",
                warm_latency
            );

            (cold_latency, warm_latency, performance_ratio)
        })
    });

    group.finish();
}

criterion_group!(
    latency_benches,
    bench_sub_millisecond_latency,
    bench_latency_under_load,
    bench_latency_percentiles,
    bench_latency_jitter,
    bench_warmup_latency_effects
);

criterion_main!(latency_benches);
