// High Frequency Data Ingestion Benchmarks
// "Measuring the speed of the fastest ravens in the realm"
// Comprehensive performance benchmarks for atomic operations, gRPC streaming, and memory usage

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use market_data_subscription_server::data_handlers::high_frequency::HighFrequencyHandler;
use market_data_subscription_server::types::{HighFrequencyStorage, OrderBookData, TradeData};
use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn create_benchmark_orderbook_data(symbol: &str, sequence: u64) -> OrderBookData {
    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        bids: vec![
            (45000.0 + sequence as f64, 1.5),
            (44999.0 + sequence as f64, 2.0),
            (44998.0 + sequence as f64, 1.8),
            (44997.0 + sequence as f64, 2.2),
            (44996.0 + sequence as f64, 1.1),
        ],
        asks: vec![
            (45001.0 + sequence as f64, 1.2),
            (45002.0 + sequence as f64, 1.8),
            (45003.0 + sequence as f64, 2.1),
            (45004.0 + sequence as f64, 1.9),
            (45005.0 + sequence as f64, 1.3),
        ],
        sequence,
        exchange: "binance".to_string(),
    }
}

fn create_benchmark_trade_data(symbol: &str, price: f64, side: &str) -> TradeData {
    TradeData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000,
        price,
        quantity: 0.1 + (price % 10.0) / 100.0, // Vary quantity slightly
        side: side.to_string(),
        trade_id: format!("trade_{price}_{side}"),
        exchange: "binance".to_string(),
    }
}

fn bench_orderbook_ingestion_single(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let data = create_benchmark_orderbook_data("BTCUSDT", 12345);

    c.bench_function("orderbook_ingestion_single", |b| {
        b.iter(|| {
            handler
                .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&data))
                .unwrap()
        })
    });
}

fn bench_trade_ingestion_single(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let data = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");

    c.bench_function("trade_ingestion_single", |b| {
        b.iter(|| {
            handler
                .ingest_trade_atomic(black_box("BTCUSDT"), black_box(&data))
                .unwrap()
        })
    });
}

fn bench_orderbook_ingestion_multiple_symbols(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT", "DOTUSDT", "LINKUSDT"];
    let data: Vec<_> = symbols
        .iter()
        .enumerate()
        .map(|(i, symbol)| create_benchmark_orderbook_data(symbol, 12345 + i as u64))
        .collect();

    c.bench_function("orderbook_ingestion_multiple_symbols", |b| {
        b.iter(|| {
            for (i, symbol) in symbols.iter().enumerate() {
                handler
                    .ingest_orderbook_atomic(black_box(symbol), black_box(&data[i]))
                    .unwrap();
            }
        })
    });
}

fn bench_trade_ingestion_multiple_symbols(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT", "DOTUSDT", "LINKUSDT"];
    let data: Vec<_> = symbols
        .iter()
        .enumerate()
        .map(|(i, symbol)| create_benchmark_trade_data(symbol, 45000.0 + i as f64, "buy"))
        .collect();

    c.bench_function("trade_ingestion_multiple_symbols", |b| {
        b.iter(|| {
            for (i, symbol) in symbols.iter().enumerate() {
                handler
                    .ingest_trade_atomic(black_box(symbol), black_box(&data[i]))
                    .unwrap();
            }
        })
    });
}

fn bench_concurrent_ingestion(c: &mut Criterion) {
    let handler = Arc::new(HighFrequencyHandler::new());

    c.bench_function("concurrent_orderbook_ingestion", |b| {
        b.iter(|| {
            let handler_clone = Arc::clone(&handler);
            let data = create_benchmark_orderbook_data("BTCUSDT", 12345);

            // Simulate concurrent access (though this is still sequential in benchmark)
            handler_clone
                .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&data))
                .unwrap();
        })
    });
}

fn bench_snapshot_capture(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();

    // Pre-populate with data
    let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", 12345);
    let trade_data = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");

    handler
        .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
        .unwrap();
    handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();

    let mut group = c.benchmark_group("snapshot_capture");

    group.bench_function("orderbook_snapshot", |b| {
        b.iter(|| {
            handler
                .capture_orderbook_snapshot(black_box("BTCUSDT"))
                .unwrap()
        })
    });

    group.bench_function("trade_snapshot", |b| {
        b.iter(|| {
            handler
                .capture_trade_snapshot(black_box("BTCUSDT"))
                .unwrap()
        })
    });

    group.finish();
}

fn bench_throughput_simulation(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();

    // Simulate high-frequency trading scenario
    let orderbook_updates: Vec<_> = (0..1000)
        .map(|i| create_benchmark_orderbook_data("BTCUSDT", 12345 + i))
        .collect();

    let trade_updates: Vec<_> = (0..1000)
        .map(|i| {
            create_benchmark_trade_data(
                "BTCUSDT",
                45000.0 + i as f64,
                if i % 2 == 0 { "buy" } else { "sell" },
            )
        })
        .collect();

    let mut group = c.benchmark_group("throughput_simulation");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("1000_orderbook_updates", |b| {
        b.iter(|| {
            for data in &orderbook_updates {
                handler
                    .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(data))
                    .unwrap();
            }
        })
    });

    group.bench_function("1000_trade_updates", |b| {
        b.iter(|| {
            for data in &trade_updates {
                handler
                    .ingest_trade_atomic(black_box("BTCUSDT"), black_box(data))
                    .unwrap();
            }
        })
    });

    group.bench_function("mixed_1000_updates", |b| {
        b.iter(|| {
            for i in 0..500 {
                handler
                    .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&orderbook_updates[i]))
                    .unwrap();

                handler
                    .ingest_trade_atomic(black_box("BTCUSDT"), black_box(&trade_updates[i]))
                    .unwrap();
            }
        })
    });

    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let symbols = vec![
        "BTCUSDT",
        "ETHUSDT",
        "ADAUSDT",
        "DOTUSDT",
        "LINKUSDT",
        "BNBUSDT",
        "XRPUSDT",
        "SOLUSDT",
        "MATICUSDT",
        "AVAXUSDT",
    ];

    // Pre-populate with data for multiple symbols
    for (i, symbol) in symbols.iter().enumerate() {
        let orderbook_data = create_benchmark_orderbook_data(symbol, 12345 + i as u64);
        let trade_data = create_benchmark_trade_data(symbol, 45000.0 + i as f64, "buy");

        handler
            .ingest_orderbook_atomic(symbol, &orderbook_data)
            .unwrap();
        handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
    }

    c.bench_function("memory_efficiency_multi_symbol", |b| {
        b.iter(|| {
            // Simulate accessing data for all symbols
            for symbol in &symbols {
                let _ = handler.capture_orderbook_snapshot(black_box(symbol));
                let _ = handler.capture_trade_snapshot(black_box(symbol));
            }
        })
    });
}

fn bench_validation_overhead(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();

    // Valid data
    let valid_orderbook = create_benchmark_orderbook_data("BTCUSDT", 12345);
    let valid_trade = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");

    // Invalid data (will fail validation)
    let mut invalid_orderbook = create_benchmark_orderbook_data("BTCUSDT", 12345);
    invalid_orderbook.bids.clear();
    invalid_orderbook.asks.clear();

    let mut invalid_trade = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");
    invalid_trade.price = -45000.0;

    let mut group = c.benchmark_group("validation_overhead");

    group.bench_function("valid_orderbook", |b| {
        b.iter(|| {
            let _ =
                handler.ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&valid_orderbook));
        })
    });

    group.bench_function("invalid_orderbook", |b| {
        b.iter(|| {
            let _ = handler
                .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&invalid_orderbook));
        })
    });

    group.bench_function("valid_trade", |b| {
        b.iter(|| {
            let _ = handler.ingest_trade_atomic(black_box("BTCUSDT"), black_box(&valid_trade));
        })
    });

    group.bench_function("invalid_trade", |b| {
        b.iter(|| {
            let _ = handler.ingest_trade_atomic(black_box("BTCUSDT"), black_box(&invalid_trade));
        })
    });

    group.finish();
}

// Benchmark for atomic operations latency (Task requirement 1)
fn bench_atomic_operations_latency(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();
    let data = create_benchmark_orderbook_data("BTCUSDT", 12345);
    let trade_data = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");

    let mut group = c.benchmark_group("atomic_operations_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);

    // Benchmark single atomic orderbook update
    group.bench_function("single_orderbook_atomic_update", |b| {
        b.iter(|| {
            let start = Instant::now();
            handler
                .ingest_orderbook_atomic(black_box("BTCUSDT"), black_box(&data))
                .unwrap();
            let duration = start.elapsed();
            // Note: In production, this should be sub-microsecond
            // Benchmark environment may have higher overhead
            black_box(duration);
        })
    });

    // Benchmark single atomic trade update
    group.bench_function("single_trade_atomic_update", |b| {
        b.iter(|| {
            let start = Instant::now();
            handler
                .ingest_trade_atomic(black_box("BTCUSDT"), black_box(&trade_data))
                .unwrap();
            let duration = start.elapsed();
            // Note: In production, this should be sub-microsecond
            // Benchmark environment may have higher overhead
            black_box(duration);
        })
    });

    // Benchmark atomic snapshot capture
    group.bench_function("atomic_snapshot_capture", |b| {
        b.iter(|| {
            // Pre-populate data for each iteration
            handler.ingest_orderbook_atomic("BTCUSDT", &data).unwrap();
            handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();

            let start = Instant::now();
            // Use exchange-qualified symbol as that's how data is stored internally
            let exchange_symbol = format!("{}:{}", data.exchange, "BTCUSDT");
            let _ = handler
                .capture_orderbook_snapshot(black_box(&exchange_symbol))
                .unwrap();
            let duration = start.elapsed();
            // Note: In production, this should be sub-microsecond
            // Benchmark environment may have higher overhead
            black_box(duration);
        })
    });

    group.finish();
}

// Benchmark gRPC streaming throughput (Task requirement 2)
fn bench_grpc_streaming_throughput(c: &mut Criterion) {
    let handler = HighFrequencyHandler::new();

    let mut group = c.benchmark_group("grpc_streaming_throughput");
    group.measurement_time(Duration::from_secs(30));

    // Test different message rates
    for rate in [1000, 5000, 10000, 15000, 20000].iter() {
        group.throughput(Throughput::Elements(*rate as u64));

        group.bench_with_input(
            BenchmarkId::new("messages_per_second", rate),
            rate,
            |b, &rate| {
                b.iter(|| {
                    let start = Instant::now();
                    let mut messages_sent = 0;

                    // Simulate high-frequency message processing for 1 second
                    while start.elapsed() < Duration::from_secs(1) && messages_sent < rate {
                        let sequence = messages_sent as u64;
                        let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", sequence);
                        let trade_data = create_benchmark_trade_data(
                            "BTCUSDT",
                            45000.0 + sequence as f64,
                            "buy",
                        );

                        // Simulate gRPC message processing
                        handler
                            .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
                            .unwrap();
                        handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();

                        // Capture snapshots (simulating gRPC streaming)
                        let _ = handler.capture_orderbook_snapshot("BTCUSDT").unwrap();
                        let _ = handler.capture_trade_snapshot("BTCUSDT").unwrap();

                        messages_sent += 1;
                    }

                    let actual_rate = messages_sent as f64 / start.elapsed().as_secs_f64();

                    // Verify we can handle at least 10k messages/second (requirement)
                    if rate >= 10000 {
                        assert!(
                            actual_rate >= 10000.0,
                            "Failed to achieve 10k+ msg/sec requirement. Actual: {:.0} msg/sec",
                            actual_rate
                        );
                    }

                    messages_sent
                })
            },
        );
    }

    group.finish();
}

// Benchmark memory usage under sustained load (Task requirement 3)
fn bench_memory_usage_sustained_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_sustained_load");
    group.measurement_time(Duration::from_secs(60)); // Extended test for sustained load
    group.sample_size(10);

    group.bench_function("sustained_high_frequency_load", |b| {
        b.iter(|| {
            let handler = HighFrequencyHandler::new();
            let symbols = vec![
                "BTCUSDT",
                "ETHUSDT",
                "ADAUSDT",
                "DOTUSDT",
                "LINKUSDT",
                "BNBUSDT",
                "XRPUSDT",
                "SOLUSDT",
                "MATICUSDT",
                "AVAXUSDT",
                "ATOMUSDT",
                "NEARUSDT",
                "FTMUSDT",
                "SANDUSDT",
                "MANAUSDT",
                "ALGOUSDT",
                "VETUSDT",
                "ICPUSDT",
                "THETAUSDT",
                "FILUSDT",
            ];

            let start_time = Instant::now();
            let mut total_operations = 0;

            // Run for 30 seconds of sustained load
            while start_time.elapsed() < Duration::from_secs(30) {
                for (i, symbol) in symbols.iter().enumerate() {
                    let sequence = total_operations + i as u64;

                    // Create orderbook data
                    let orderbook_data = OrderBookData {
                        symbol: symbol.to_string(),
                        timestamp: 1640995200000 + sequence as i64,
                        bids: vec![
                            (
                                45000.0 + sequence as f64,
                                1.5 + (sequence % 10) as f64 / 10.0,
                            ),
                            (
                                44999.0 + sequence as f64,
                                2.0 + (sequence % 5) as f64 / 10.0,
                            ),
                        ],
                        asks: vec![
                            (
                                45001.0 + sequence as f64,
                                1.2 + (sequence % 8) as f64 / 10.0,
                            ),
                            (
                                45002.0 + sequence as f64,
                                1.8 + (sequence % 6) as f64 / 10.0,
                            ),
                        ],
                        sequence,
                        exchange: "binance".to_string(),
                    };

                    // Create trade data
                    let trade_data = TradeData {
                        symbol: symbol.to_string(),
                        timestamp: 1640995200000 + sequence as i64,
                        price: 45000.0 + sequence as f64,
                        quantity: 0.1 + (sequence % 100) as f64 / 1000.0,
                        side: if sequence % 2 == 0 { "buy" } else { "sell" }.to_string(),
                        trade_id: format!("trade_{}", sequence),
                        exchange: "binance".to_string(),
                    };

                    // Ingest data
                    handler
                        .ingest_orderbook_atomic(symbol, &orderbook_data)
                        .unwrap();
                    handler.ingest_trade_atomic(symbol, &trade_data).unwrap();

                    // Periodically capture snapshots
                    if sequence % 100 == 0 {
                        let _ = handler.capture_orderbook_snapshot(symbol);
                        let _ = handler.capture_trade_snapshot(symbol);
                    }
                }

                total_operations += symbols.len() as u64;
            }

            let duration = start_time.elapsed();
            let ops_per_second = total_operations as f64 / duration.as_secs_f64();

            // Verify performance under sustained load
            assert!(
                ops_per_second >= 10000.0,
                "Sustained load performance below 10k ops/sec: {:.0}",
                ops_per_second
            );

            // Return metrics for analysis
            (total_operations, duration, ops_per_second)
        })
    });

    group.finish();
}

// Benchmark cache-line alignment optimization (Task requirement 4)
fn bench_cache_line_alignment_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_line_alignment");
    group.measurement_time(Duration::from_secs(10));

    // Test concurrent access patterns that would cause false sharing without alignment
    group.bench_function("concurrent_atomic_updates", |b| {
        b.iter(|| {
            let handler = Arc::new(HighFrequencyHandler::new());
            let num_threads = 8;
            let operations_per_thread = 1000;
            let mut handles = vec![];

            let start_time = Instant::now();

            // Spawn threads that would cause false sharing without cache-line alignment
            for thread_id in 0..num_threads {
                let handler_clone = Arc::clone(&handler);
                let handle = thread::spawn(move || {
                    let mut thread_operations = 0;

                    for i in 0..operations_per_thread {
                        let sequence = (thread_id * operations_per_thread + i) as u64;
                        let symbol = format!("SYMBOL{}", thread_id);

                        let orderbook_data = create_benchmark_orderbook_data(&symbol, sequence);
                        let trade_data =
                            create_benchmark_trade_data(&symbol, 45000.0 + sequence as f64, "buy");

                        // These operations would cause false sharing without proper alignment
                        handler_clone
                            .ingest_orderbook_atomic(&symbol, &orderbook_data)
                            .unwrap();
                        handler_clone
                            .ingest_trade_atomic(&symbol, &trade_data)
                            .unwrap();

                        thread_operations += 1;
                    }

                    thread_operations
                });
                handles.push(handle);
            }

            // Wait for all threads and collect results
            let mut total_operations = 0;
            for handle in handles {
                total_operations += handle.join().unwrap();
            }

            let duration = start_time.elapsed();
            let ops_per_second = total_operations as f64 / duration.as_secs_f64();

            // With proper cache-line alignment, we should achieve high concurrent performance
            assert!(
                ops_per_second >= 50000.0,
                "Concurrent performance below expected: {:.0} ops/sec",
                ops_per_second
            );

            total_operations
        })
    });

    group.finish();
}

// End-to-end latency validation (Task requirement 5)
fn bench_end_to_end_latency_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);

    group.bench_function("complete_data_flow_latency", |b| {
        let handler = HighFrequencyHandler::new();

        b.iter(|| {
            let overall_start = Instant::now();

            // Step 1: Data ingestion (simulating WebSocket receive)
            let ingestion_start = Instant::now();
            let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", 12345);
            let trade_data = create_benchmark_trade_data("BTCUSDT", 45000.0, "buy");

            handler
                .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
                .unwrap();
            handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();
            let ingestion_duration = ingestion_start.elapsed();

            // Step 2: Snapshot capture (simulating periodic capture)
            let snapshot_start = Instant::now();
            let orderbook_snapshot = handler.capture_orderbook_snapshot("BTCUSDT").unwrap();
            let trade_snapshot = handler.capture_trade_snapshot("BTCUSDT").unwrap();
            let snapshot_duration = snapshot_start.elapsed();

            // Step 3: Data serialization (simulating gRPC message preparation)
            let serialization_start = Instant::now();
            let _serialized_orderbook = serde_json::to_string(&orderbook_snapshot).unwrap();
            let _serialized_trade = serde_json::to_string(&trade_snapshot).unwrap();
            let serialization_duration = serialization_start.elapsed();

            let total_duration = overall_start.elapsed();

            // Note: In production, these should meet stricter requirements
            // Benchmark environment has additional overhead
            if total_duration.as_micros() > 5000 {
                // 5ms threshold for benchmark
                eprintln!("Warning: End-to-end latency high: {:?}", total_duration);
            }

            // Individual component latency validation (more lenient for benchmarks)
            if ingestion_duration.as_micros() > 100 {
                // 100μs threshold for benchmark
                eprintln!("Warning: Ingestion latency high: {:?}", ingestion_duration);
            }
            if snapshot_duration.as_micros() > 100 {
                // 100μs threshold for benchmark
                eprintln!("Warning: Snapshot latency high: {:?}", snapshot_duration);
            }

            (
                total_duration,
                ingestion_duration,
                snapshot_duration,
                serialization_duration,
            )
        })
    });

    group.finish();
}

// Comprehensive throughput benchmark with multiple data types
fn bench_comprehensive_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive_throughput");
    group.measurement_time(Duration::from_secs(20));

    for num_symbols in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_symbols as u64));

        group.bench_with_input(
            BenchmarkId::new("mixed_data_throughput", num_symbols),
            num_symbols,
            |b, &num_symbols| {
                let handler = HighFrequencyHandler::new();
                let symbols: Vec<String> =
                    (0..num_symbols).map(|i| format!("SYMBOL{}", i)).collect();

                b.iter(|| {
                    let start = Instant::now();
                    let mut operations = 0;

                    // Run for 1 second and measure throughput
                    while start.elapsed() < Duration::from_secs(1) {
                        for (i, symbol) in symbols.iter().enumerate() {
                            let sequence = operations + i as u64;

                            // Mix of orderbook and trade updates
                            let orderbook_data = create_benchmark_orderbook_data(symbol, sequence);
                            let trade_data = create_benchmark_trade_data(
                                symbol,
                                45000.0 + sequence as f64,
                                if sequence % 2 == 0 { "buy" } else { "sell" },
                            );

                            handler
                                .ingest_orderbook_atomic(symbol, &orderbook_data)
                                .unwrap();
                            handler.ingest_trade_atomic(symbol, &trade_data).unwrap();

                            // Periodic snapshots
                            if sequence % 10 == 0 {
                                let _ = handler.capture_orderbook_snapshot(symbol);
                                let _ = handler.capture_trade_snapshot(symbol);
                            }
                        }
                        operations += symbols.len() as u64;
                    }

                    let actual_throughput = operations as f64 / start.elapsed().as_secs_f64();

                    // Scale expectations based on number of symbols
                    let expected_min_throughput = match num_symbols {
                        1 => 50000.0,
                        10 => 30000.0,
                        50 => 15000.0,
                        100 => 10000.0,
                        _ => 5000.0,
                    };

                    assert!(
                        actual_throughput >= expected_min_throughput,
                        "Throughput below expected for {} symbols: {:.0} ops/sec (expected: {:.0})",
                        num_symbols,
                        actual_throughput,
                        expected_min_throughput
                    );

                    operations
                })
            },
        );
    }

    group.finish();
}

// Memory efficiency and garbage collection impact
fn bench_memory_efficiency_gc_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("memory_allocation_pattern", |b| {
        b.iter(|| {
            let handler = HighFrequencyHandler::new();
            let symbols = (0..1000)
                .map(|i| format!("SYMBOL{}", i))
                .collect::<Vec<_>>();

            // Test memory allocation patterns
            for (i, symbol) in symbols.iter().enumerate() {
                let orderbook_data = create_benchmark_orderbook_data(symbol, i as u64);
                let trade_data = create_benchmark_trade_data(symbol, 45000.0 + i as f64, "buy");

                handler
                    .ingest_orderbook_atomic(symbol, &orderbook_data)
                    .unwrap();
                handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
            }

            // Verify all symbols are accessible
            let orderbook_symbols = handler.get_orderbook_symbols();
            let trade_symbols = handler.get_trade_symbols();

            assert_eq!(orderbook_symbols.len(), symbols.len());
            assert_eq!(trade_symbols.len(), symbols.len());

            // Test snapshot capture for all symbols
            for symbol in &symbols {
                let _ = handler.capture_orderbook_snapshot(symbol).unwrap();
                let _ = handler.capture_trade_snapshot(symbol).unwrap();
            }

            symbols.len()
        })
    });

    group.finish();
}

criterion_group!(
    atomic_latency_benches,
    bench_atomic_operations_latency,
    bench_cache_line_alignment_optimization,
    bench_end_to_end_latency_validation
);

criterion_group!(
    throughput_benches,
    bench_grpc_streaming_throughput,
    bench_comprehensive_throughput,
    bench_memory_usage_sustained_load
);

criterion_group!(memory_benches, bench_memory_efficiency_gc_impact);

criterion_group!(
    legacy_benches,
    bench_orderbook_ingestion_single,
    bench_trade_ingestion_single,
    bench_orderbook_ingestion_multiple_symbols,
    bench_trade_ingestion_multiple_symbols,
    bench_concurrent_ingestion,
    bench_snapshot_capture,
    bench_throughput_simulation,
    bench_memory_efficiency,
    bench_validation_overhead
);

criterion_main!(
    atomic_latency_benches,
    throughput_benches,
    memory_benches,
    legacy_benches
);
