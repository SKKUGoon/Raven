// gRPC Streaming Performance Benchmarks
// "Testing the ravens' ability to carry messages across the realm at scale"

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use market_data_subscription_server::data_handlers::high_frequency::HighFrequencyHandler;
use market_data_subscription_server::types::{OrderBookData, TradeData};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;
use tokio::sync::mpsc;
use tokio::time::sleep;
use rand::Rng;

fn create_benchmark_orderbook_data(symbol: &str, sequence: u64) -> OrderBookData {
    let mut rng = rand::thread_rng();
    let base_price = 45000.0 + (sequence % 1000) as f64;
    
    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        bids: vec![
            (base_price - rng.gen_range(0.1..1.0), rng.gen_range(0.1..5.0)),
            (base_price - rng.gen_range(1.0..2.0), rng.gen_range(0.1..5.0)),
            (base_price - rng.gen_range(2.0..3.0), rng.gen_range(0.1..5.0)),
        ],
        asks: vec![
            (base_price + rng.gen_range(0.1..1.0), rng.gen_range(0.1..5.0)),
            (base_price + rng.gen_range(1.0..2.0), rng.gen_range(0.1..5.0)),
            (base_price + rng.gen_range(2.0..3.0), rng.gen_range(0.1..5.0)),
        ],
        sequence,
        exchange: "binance".to_string(),
    }
}

fn create_benchmark_trade_data(symbol: &str, sequence: u64, side: &str) -> TradeData {
    let mut rng = rand::thread_rng();
    
    TradeData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        price: 45000.0 + (sequence % 1000) as f64 + rng.gen_range(-10.0..10.0),
        quantity: rng.gen_range(0.001..10.0),
        side: side.to_string(),
        trade_id: format!("trade_{}", sequence),
        exchange: "binance".to_string(),
    }
}

// Benchmark gRPC streaming with 10k+ messages per second
fn bench_grpc_streaming_10k_plus(c: &mut Criterion) {
    let mut group = c.benchmark_group("grpc_streaming_10k_plus");
    group.measurement_time(Duration::from_secs(30));
    
    // Test different message rates to validate 10k+ requirement
    for target_rate in [10000, 15000, 20000, 25000, 30000].iter() {
        group.throughput(Throughput::Elements(*target_rate as u64));
        
        group.bench_with_input(
            BenchmarkId::new("streaming_messages_per_second", target_rate),
            target_rate,
            |b, &target_rate| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                
                b.iter(|| {
                    rt.block_on(async {
                        let handler = Arc::new(HighFrequencyHandler::new());
                        let (tx, mut rx) = mpsc::unbounded_channel();
                        
                        let handler_clone = Arc::clone(&handler);
                        
                        // Simulate gRPC streaming server
                        let streaming_task = tokio::spawn(async move {
                            let mut messages_processed = 0;
                            let start_time = Instant::now();
                            
                            while let Some((orderbook_data, trade_data)) = rx.recv().await {
                                // Simulate gRPC message processing
                                handler_clone.ingest_orderbook_atomic("BTCUSDT", &orderbook_data).unwrap();
                                handler_clone.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();
                                
                                // Capture snapshots for streaming
                                let _ = handler_clone.capture_orderbook_snapshot("BTCUSDT").unwrap();
                                let _ = handler_clone.capture_trade_snapshot("BTCUSDT").unwrap();
                                
                                messages_processed += 1;
                                
                                // Stop after processing target number of messages or 10 seconds
                                if messages_processed >= target_rate || start_time.elapsed() > Duration::from_secs(10) {
                                    break;
                                }
                            }
                            
                            (messages_processed, start_time.elapsed())
                        });
                        
                        // Simulate high-frequency data producer
                        let producer_task = tokio::spawn(async move {
                            let mut sequence = 0;
                            let start_time = Instant::now();
                            let target_interval = Duration::from_nanos(1_000_000_000 / target_rate as u64);
                            
                            while sequence < target_rate as u64 && start_time.elapsed() < Duration::from_secs(10) {
                                let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", sequence);
                                let trade_data = create_benchmark_trade_data("BTCUSDT", sequence, 
                                    if sequence % 2 == 0 { "buy" } else { "sell" });
                                
                                if tx.send((orderbook_data, trade_data)).is_err() {
                                    break;
                                }
                                
                                sequence += 1;
                                
                                // Rate limiting to achieve target throughput
                                if sequence % 100 == 0 {
                                    sleep(target_interval * 100).await;
                                }
                            }
                        });
                        
                        // Wait for both tasks to complete
                        let (producer_result, streaming_result) = tokio::join!(producer_task, streaming_task);
                        
                        producer_result.unwrap();
                        let (messages_processed, duration) = streaming_result.unwrap();
                        
                        let actual_rate = messages_processed as f64 / duration.as_secs_f64();
                        
                        // Validate 10k+ messages per second requirement
                        if target_rate >= 10000 {
                            assert!(actual_rate >= 10000.0,
                                "Failed to achieve 10k+ msg/sec requirement. Target: {}, Actual: {:.0} msg/sec",
                                target_rate, actual_rate);
                        }
                        
                        messages_processed
                    })
                })
            }
        );
    }
    
    group.finish();
}

// Benchmark concurrent gRPC clients
fn bench_concurrent_grpc_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_grpc_clients");
    group.measurement_time(Duration::from_secs(20));
    
    for num_clients in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*num_clients as u64));
        
        group.bench_with_input(
            BenchmarkId::new("concurrent_clients", num_clients),
            num_clients,
            |b, &num_clients| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                
                b.iter(|| {
                    rt.block_on(async {
                        let handler = Arc::new(HighFrequencyHandler::new());
                        let mut client_tasks = vec![];
                        
                        // Spawn multiple concurrent client simulations
                        for client_id in 0..num_clients {
                            let handler_clone = Arc::clone(&handler);
                            let symbol = format!("SYMBOL{}", client_id % 10); // 10 different symbols
                            
                            let client_task = tokio::spawn(async move {
                                let mut messages_processed = 0;
                                let start_time = Instant::now();
                                
                                // Each client processes messages for 5 seconds
                                while start_time.elapsed() < Duration::from_secs(5) {
                                    let sequence = messages_processed;
                                    
                                    let orderbook_data = create_benchmark_orderbook_data(&symbol, sequence);
                                    let trade_data = create_benchmark_trade_data(&symbol, sequence, "buy");
                                    
                                    // Simulate client-specific processing
                                    handler_clone.ingest_orderbook_atomic(&symbol, &orderbook_data).unwrap();
                                    handler_clone.ingest_trade_atomic(&symbol, &trade_data).unwrap();
                                    
                                    // Client reads snapshots
                                    let _ = handler_clone.capture_orderbook_snapshot(&symbol);
                                    let _ = handler_clone.capture_trade_snapshot(&symbol);
                                    
                                    messages_processed += 1;
                                    
                                    // Small delay to simulate realistic client behavior
                                    if messages_processed % 100 == 0 {
                                        sleep(Duration::from_micros(100)).await;
                                    }
                                }
                                
                                messages_processed
                            });
                            
                            client_tasks.push(client_task);
                        }
                        
                        // Wait for all clients to complete
                        let mut total_messages = 0;
                        for task in client_tasks {
                            total_messages += task.await.unwrap();
                        }
                        
                        // Validate concurrent performance
                        let messages_per_client = total_messages / num_clients as u64;
                        
                        // Each client should process at least 1000 messages in 5 seconds
                        assert!(messages_per_client >= 1000,
                            "Concurrent client performance below expected: {} messages per client",
                            messages_per_client);
                        
                        total_messages
                    })
                })
            }
        );
    }
    
    group.finish();
}

// Benchmark streaming with backpressure handling
fn bench_streaming_backpressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_backpressure");
    group.measurement_time(Duration::from_secs(15));
    
    group.bench_function("backpressure_handling", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        b.iter(|| {
            rt.block_on(async {
                let handler = Arc::new(HighFrequencyHandler::new());
                let (tx, mut rx) = mpsc::channel(1000); // Limited buffer to create backpressure
                
                let handler_clone = Arc::clone(&handler);
                
                // Slow consumer to create backpressure
                let consumer_task = tokio::spawn(async move {
                    let mut messages_consumed = 0;
                    
                    while let Some((orderbook_data, trade_data)) = rx.recv().await {
                        // Simulate slow processing
                        handler_clone.ingest_orderbook_atomic("BTCUSDT", &orderbook_data).unwrap();
                        handler_clone.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();
                        
                        messages_consumed += 1;
                        
                        // Simulate slow consumer
                        sleep(Duration::from_micros(200)).await;
                        
                        if messages_consumed >= 1000 {
                            break;
                        }
                    }
                    
                    messages_consumed
                });
                
                // Fast producer to create backpressure
                let producer_task = tokio::spawn(async move {
                    let mut sequence = 0;
                    let mut messages_sent = 0;
                    
                    while messages_sent < 2000 {
                        let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", sequence);
                        let trade_data = create_benchmark_trade_data("BTCUSDT", sequence, "buy");
                        
                        // This will block when buffer is full (backpressure)
                        match tx.send((orderbook_data, trade_data)).await {
                            Ok(_) => {
                                messages_sent += 1;
                                sequence += 1;
                            }
                            Err(_) => break,
                        }
                    }
                    
                    messages_sent
                });
                
                let (producer_result, consumer_result) = tokio::join!(producer_task, consumer_task);
                
                let messages_sent = producer_result.unwrap();
                let messages_consumed = consumer_result.unwrap();
                
                // Validate backpressure handling
                assert!(messages_consumed > 0, "No messages were consumed");
                assert!(messages_sent >= messages_consumed, "More messages consumed than sent");
                
                (messages_sent, messages_consumed)
            })
        })
    });
    
    group.finish();
}

// Benchmark message serialization performance for gRPC
fn bench_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");
    group.measurement_time(Duration::from_secs(10));
    
    let handler = HighFrequencyHandler::new();
    
    // Pre-populate with data
    let orderbook_data = create_benchmark_orderbook_data("BTCUSDT", 12345);
    let trade_data = create_benchmark_trade_data("BTCUSDT", 12345, "buy");
    
    handler.ingest_orderbook_atomic("BTCUSDT", &orderbook_data).unwrap();
    handler.ingest_trade_atomic("BTCUSDT", &trade_data).unwrap();
    
    group.bench_function("json_serialization", |b| {
        b.iter(|| {
            let orderbook_snapshot = handler.capture_orderbook_snapshot(black_box("BTCUSDT")).unwrap();
            let trade_snapshot = handler.capture_trade_snapshot(black_box("BTCUSDT")).unwrap();
            
            let serialized_orderbook = serde_json::to_string(&orderbook_snapshot).unwrap();
            let serialized_trade = serde_json::to_string(&trade_snapshot).unwrap();
            
            (serialized_orderbook.len(), serialized_trade.len())
        })
    });
    
    group.bench_function("binary_serialization", |b| {
        b.iter(|| {
            let orderbook_snapshot = handler.capture_orderbook_snapshot(black_box("BTCUSDT")).unwrap();
            let trade_snapshot = handler.capture_trade_snapshot(black_box("BTCUSDT")).unwrap();
            
            let serialized_orderbook = bincode::serialize(&orderbook_snapshot).unwrap();
            let serialized_trade = bincode::serialize(&trade_snapshot).unwrap();
            
            (serialized_orderbook.len(), serialized_trade.len())
        })
    });
    
    group.finish();
}

criterion_group!(
    grpc_streaming_benches,
    bench_grpc_streaming_10k_plus,
    bench_concurrent_grpc_clients,
    bench_streaming_backpressure,
    bench_message_serialization
);

criterion_main!(grpc_streaming_benches);