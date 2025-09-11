// Memory Usage and Performance Benchmarks
// "Testing the ravens' ability to manage their memory efficiently under sustained load"

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use market_data_subscription_server::data_handlers::high_frequency::HighFrequencyHandler;
use market_data_subscription_server::types::{HighFrequencyStorage, OrderBookData, TradeData};
use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn create_realistic_orderbook_data(symbol: &str, sequence: u64) -> OrderBookData {
    let mut rng = rand::thread_rng();
    let base_price = 45000.0 + (sequence as f64 * 0.01) + rng.gen_range(-50.0..50.0);

    // Create realistic orderbook with multiple price levels
    let mut bids = Vec::new();
    let mut asks = Vec::new();

    // Generate 10 bid levels
    for i in 0..10 {
        let price = base_price - (i as f64 * 0.1) - rng.gen_range(0.01..0.1);
        let quantity = rng.gen_range(0.1..10.0);
        bids.push((price, quantity));
    }

    // Generate 10 ask levels
    for i in 0..10 {
        let price = base_price + (i as f64 * 0.1) + rng.gen_range(0.01..0.1);
        let quantity = rng.gen_range(0.1..10.0);
        asks.push((price, quantity));
    }

    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        bids,
        asks,
        sequence,
        exchange: "binance".to_string(),
    }
}

fn create_realistic_trade_data(symbol: &str, sequence: u64) -> TradeData {
    let mut rng = rand::thread_rng();

    TradeData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        price: 45000.0 + (sequence as f64 * 0.01) + rng.gen_range(-10.0..10.0),
        quantity: rng.gen_range(0.001..5.0),
        side: if rng.gen_bool(0.5) { "buy" } else { "sell" }.to_string(),
        trade_id: format!("trade_{}_{}", symbol, sequence),
        exchange: "binance".to_string(),
    }
}

// Test memory usage under sustained high-frequency load
fn bench_sustained_memory_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_memory_load");
    group.measurement_time(Duration::from_secs(60)); // Extended test for memory analysis
    group.sample_size(10);

    // Test different numbers of symbols to analyze memory scaling
    for num_symbols in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Elements(*num_symbols as u64));

        group.bench_with_input(
            BenchmarkId::new("symbols_memory_usage", num_symbols),
            num_symbols,
            |b, &num_symbols| {
                b.iter(|| {
                    let handler = HighFrequencyHandler::new();
                    let symbols: Vec<String> = (0..num_symbols)
                        .map(|i| format!("SYMBOL{:04}", i))
                        .collect();

                    let start_time = Instant::now();
                    let mut total_operations = 0u64;
                    let operations_counter = Arc::new(AtomicU64::new(0));

                    // Run sustained load for 30 seconds
                    while start_time.elapsed() < Duration::from_secs(30) {
                        for (i, symbol) in symbols.iter().enumerate() {
                            let sequence = total_operations + i as u64;

                            // Create realistic market data
                            let orderbook_data = create_realistic_orderbook_data(symbol, sequence);
                            let trade_data = create_realistic_trade_data(symbol, sequence);

                            // Ingest data (this tests memory allocation patterns)
                            handler
                                .ingest_orderbook_atomic(symbol, &orderbook_data)
                                .unwrap();
                            handler.ingest_trade_atomic(symbol, &trade_data).unwrap();

                            // Periodically capture snapshots (tests memory access patterns)
                            if sequence % 100 == 0 {
                                let _ = handler.capture_orderbook_snapshot(symbol);
                                let _ = handler.capture_trade_snapshot(symbol);
                            }

                            operations_counter.fetch_add(1, Ordering::Relaxed);
                        }

                        total_operations += symbols.len() as u64;

                        // Brief pause to allow memory management
                        if total_operations % 10000 == 0 {
                            thread::sleep(Duration::from_millis(1));
                        }
                    }

                    let duration = start_time.elapsed();
                    let ops_per_second = total_operations as f64 / duration.as_secs_f64();

                    // Verify performance doesn't degrade significantly with more symbols
                    let expected_min_ops_per_sec = match num_symbols {
                        100 => 20000.0,
                        500 => 15000.0,
                        1000 => 10000.0,
                        2000 => 8000.0,
                        _ => 5000.0,
                    };

                    assert!(
                        ops_per_second >= expected_min_ops_per_sec,
                        "Performance degraded with {} symbols: {:.0} ops/sec (expected: {:.0})",
                        num_symbols,
                        ops_per_second,
                        expected_min_ops_per_sec
                    );

                    // Verify all symbols are still accessible (memory integrity)
                    let orderbook_symbols = handler.get_orderbook_symbols();
                    let trade_symbols = handler.get_trade_symbols();

                    assert_eq!(orderbook_symbols.len(), num_symbols);
                    assert_eq!(trade_symbols.len(), num_symbols);

                    // Test random access to verify memory layout efficiency
                    let mut rng = rand::thread_rng();
                    for _ in 0..100 {
                        let random_symbol = &symbols[rng.gen_range(0..symbols.len())];
                        let _ = handler.capture_orderbook_snapshot(random_symbol).unwrap();
                        let _ = handler.capture_trade_snapshot(random_symbol).unwrap();
                    }

                    (
                        total_operations,
                        ops_per_second,
                        orderbook_symbols.len(),
                        trade_symbols.len(),
                    )
                })
            },
        );
    }

    group.finish();
}

// Test memory allocation patterns and fragmentation
fn bench_memory_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation_patterns");
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("allocation_deallocation_cycles", |b| {
        b.iter(|| {
            let mut handlers = Vec::new();
            let symbols = (0..100).map(|i| format!("SYMBOL{}", i)).collect::<Vec<_>>();

            // Test allocation patterns
            for cycle in 0..10 {
                let handler = HighFrequencyHandler::new();

                // Populate with data
                for (i, symbol) in symbols.iter().enumerate() {
                    let sequence = (cycle * 100 + i) as u64;
                    let orderbook_data = create_realistic_orderbook_data(symbol, sequence);
                    let trade_data = create_realistic_trade_data(symbol, sequence);

                    handler
                        .ingest_orderbook_atomic(symbol, &orderbook_data)
                        .unwrap();
                    handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
                }

                // Verify data integrity
                for symbol in &symbols {
                    let _ = handler.capture_orderbook_snapshot(symbol).unwrap();
                    let _ = handler.capture_trade_snapshot(symbol).unwrap();
                }

                handlers.push(handler);

                // Periodically drop some handlers to test deallocation
                if cycle % 3 == 0 && !handlers.is_empty() {
                    handlers.remove(0);
                }
            }

            // Final verification
            for handler in &handlers {
                let orderbook_symbols = handler.get_orderbook_symbols();
                let trade_symbols = handler.get_trade_symbols();
                assert_eq!(orderbook_symbols.len(), symbols.len());
                assert_eq!(trade_symbols.len(), symbols.len());
            }

            handlers.len()
        })
    });

    group.finish();
}

// Test concurrent memory access patterns
fn bench_concurrent_memory_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_memory_access");
    group.measurement_time(Duration::from_secs(15));

    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_threads", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let handler = Arc::new(HighFrequencyHandler::new());
                    let mut thread_handles = vec![];
                    let operations_per_thread = 5000;

                    // Spawn concurrent threads accessing shared memory
                    for thread_id in 0..num_threads {
                        let handler_clone = Arc::clone(&handler);

                        let handle = thread::spawn(move || {
                            let mut thread_operations = 0;
                            let symbols = (0..10)
                                .map(|i| format!("THREAD{}_SYMBOL{}", thread_id, i))
                                .collect::<Vec<_>>();

                            for operation in 0..operations_per_thread {
                                for (i, symbol) in symbols.iter().enumerate() {
                                    let sequence = (thread_id * operations_per_thread + operation)
                                        as u64
                                        + i as u64;

                                    let orderbook_data =
                                        create_realistic_orderbook_data(symbol, sequence);
                                    let trade_data = create_realistic_trade_data(symbol, sequence);

                                    // Concurrent writes
                                    handler_clone
                                        .ingest_orderbook_atomic(symbol, &orderbook_data)
                                        .unwrap();
                                    handler_clone
                                        .ingest_trade_atomic(symbol, &trade_data)
                                        .unwrap();

                                    // Concurrent reads
                                    if operation % 10 == 0 {
                                        let _ = handler_clone.capture_orderbook_snapshot(symbol);
                                        let _ = handler_clone.capture_trade_snapshot(symbol);
                                    }

                                    thread_operations += 1;
                                }
                            }

                            thread_operations
                        });

                        thread_handles.push(handle);
                    }

                    // Wait for all threads and collect results
                    let mut total_operations = 0;
                    for handle in thread_handles {
                        total_operations += handle.join().unwrap();
                    }

                    // Verify data integrity after concurrent access
                    let orderbook_symbols = handler.get_orderbook_symbols();
                    let trade_symbols = handler.get_trade_symbols();

                    // Each thread creates 10 symbols
                    let expected_symbols = num_threads * 10;
                    assert_eq!(orderbook_symbols.len(), expected_symbols);
                    assert_eq!(trade_symbols.len(), expected_symbols);

                    // Test random access to verify memory consistency
                    let mut rng = rand::thread_rng();
                    for _ in 0..100 {
                        if let Some(symbol) =
                            orderbook_symbols.get(rng.gen_range(0..orderbook_symbols.len()))
                        {
                            let _ = handler.capture_orderbook_snapshot(symbol).unwrap();
                        }
                        if let Some(symbol) =
                            trade_symbols.get(rng.gen_range(0..trade_symbols.len()))
                        {
                            let _ = handler.capture_trade_snapshot(symbol).unwrap();
                        }
                    }

                    total_operations
                })
            },
        );
    }

    group.finish();
}

// Test memory efficiency with large datasets
fn bench_large_dataset_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_dataset_memory");
    group.measurement_time(Duration::from_secs(30));

    group.bench_function("large_symbol_set_efficiency", |b| {
        b.iter(|| {
            let handler = HighFrequencyHandler::new();
            let num_symbols = 5000; // Large number of symbols
            let updates_per_symbol = 100;

            let symbols: Vec<String> = (0..num_symbols)
                .map(|i| format!("LARGE_SYMBOL_{:05}", i))
                .collect();

            let start_time = Instant::now();

            // Populate with large dataset
            for (symbol_idx, symbol) in symbols.iter().enumerate() {
                for update in 0..updates_per_symbol {
                    let sequence = (symbol_idx * updates_per_symbol + update) as u64;

                    let orderbook_data = create_realistic_orderbook_data(symbol, sequence);
                    let trade_data = create_realistic_trade_data(symbol, sequence);

                    handler
                        .ingest_orderbook_atomic(symbol, &orderbook_data)
                        .unwrap();
                    handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
                }
            }

            let population_time = start_time.elapsed();

            // Test access patterns on large dataset
            let access_start = Instant::now();
            let mut rng = rand::thread_rng();

            for _ in 0..1000 {
                let random_symbol = &symbols[rng.gen_range(0..symbols.len())];
                let _ = handler.capture_orderbook_snapshot(random_symbol).unwrap();
                let _ = handler.capture_trade_snapshot(random_symbol).unwrap();
            }

            let access_time = access_start.elapsed();

            // Verify data integrity
            let orderbook_symbols = handler.get_orderbook_symbols();
            let trade_symbols = handler.get_trade_symbols();

            assert_eq!(orderbook_symbols.len(), num_symbols);
            assert_eq!(trade_symbols.len(), num_symbols);

            // Performance assertions
            let population_ops_per_sec =
                (num_symbols * updates_per_symbol * 2) as f64 / population_time.as_secs_f64();
            let access_ops_per_sec = 2000.0 / access_time.as_secs_f64(); // 1000 snapshots * 2 types

            assert!(
                population_ops_per_sec >= 10000.0,
                "Large dataset population too slow: {:.0} ops/sec",
                population_ops_per_sec
            );
            assert!(
                access_ops_per_sec >= 100000.0,
                "Large dataset access too slow: {:.0} ops/sec",
                access_ops_per_sec
            );

            (num_symbols, population_ops_per_sec, access_ops_per_sec)
        })
    });

    group.finish();
}

// Test memory usage with different data patterns
fn bench_memory_data_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_data_patterns");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("sparse_vs_dense_updates", |b| {
        b.iter(|| {
            let handler = HighFrequencyHandler::new();
            let symbols: Vec<String> = (0..1000).map(|i| format!("PATTERN_SYMBOL_{}", i)).collect();

            // Test sparse update pattern (few symbols, many updates)
            let sparse_start = Instant::now();
            for update in 0..10000 {
                let symbol = &symbols[update % 10]; // Only use first 10 symbols
                let orderbook_data = create_realistic_orderbook_data(symbol, update as u64);
                let trade_data = create_realistic_trade_data(symbol, update as u64);

                handler
                    .ingest_orderbook_atomic(symbol, &orderbook_data)
                    .unwrap();
                handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
            }
            let sparse_time = sparse_start.elapsed();

            // Test dense update pattern (many symbols, few updates each)
            let dense_start = Instant::now();
            for (i, symbol) in symbols.iter().enumerate() {
                for update in 0..10 {
                    let sequence = (i * 10 + update) as u64;
                    let orderbook_data = create_realistic_orderbook_data(symbol, sequence);
                    let trade_data = create_realistic_trade_data(symbol, sequence);

                    handler
                        .ingest_orderbook_atomic(symbol, &orderbook_data)
                        .unwrap();
                    handler.ingest_trade_atomic(symbol, &trade_data).unwrap();
                }
            }
            let dense_time = dense_start.elapsed();

            // Verify both patterns work correctly
            let orderbook_symbols = handler.get_orderbook_symbols();
            let trade_symbols = handler.get_trade_symbols();

            assert_eq!(orderbook_symbols.len(), symbols.len());
            assert_eq!(trade_symbols.len(), symbols.len());

            let sparse_ops_per_sec = 20000.0 / sparse_time.as_secs_f64(); // 10k updates * 2 operations
            let dense_ops_per_sec = 20000.0 / dense_time.as_secs_f64(); // 1k symbols * 10 updates * 2 operations

            // Both patterns should maintain good performance
            assert!(
                sparse_ops_per_sec >= 50000.0,
                "Sparse pattern performance too low: {:.0} ops/sec",
                sparse_ops_per_sec
            );
            assert!(
                dense_ops_per_sec >= 50000.0,
                "Dense pattern performance too low: {:.0} ops/sec",
                dense_ops_per_sec
            );

            (sparse_ops_per_sec, dense_ops_per_sec)
        })
    });

    group.finish();
}

criterion_group!(
    memory_usage_benches,
    bench_sustained_memory_load,
    bench_memory_allocation_patterns,
    bench_concurrent_memory_access,
    bench_large_dataset_memory_efficiency,
    bench_memory_data_patterns
);

criterion_main!(memory_usage_benches);
