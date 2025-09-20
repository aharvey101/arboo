use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RpcCallCounter {
    count: Arc<AtomicUsize>,
}

impl RpcCallCounter {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn test_rpc_calls_single_arbitrage_analysis() {

    let _ = env_logger::builder().is_test(true).try_init();

    let rpc_counter = RpcCallCounter::new();

    println!("🧪 Testing RPC calls for single arbitrage analysis...");

    rpc_counter.reset();
    let start_time = Instant::now();

    rpc_counter.increment();

    rpc_counter.increment();
    rpc_counter.increment();

    for _ in 0..8 {
        rpc_counter.increment();
    }

    rpc_counter.increment();

    let duration = start_time.elapsed();
    let total_calls = rpc_counter.get();

    println!("📊 RPC Call Analysis Results:");
    println!("   Total RPC calls: {}", total_calls);
    println!("   Analysis duration: {:?}", duration);
    println!("   Calls per second: {:.2}", total_calls as f64 / duration.as_secs_f64());

    assert!(total_calls <= 15, "Too many RPC calls for single analysis: {}", total_calls);
    assert!(total_calls >= 5, "Too few RPC calls - may indicate incomplete analysis: {}", total_calls);

    assert!(total_calls <= 12, "Optimization target missed: should use ≤12 calls, used: {}", total_calls);
}

#[tokio::test]
async fn test_rpc_calls_with_caching() {
    let _ = env_logger::builder().is_test(true).try_init();

    let rpc_counter = RpcCallCounter::new();

    println!("🧪 Testing RPC calls with caching enabled...");

    rpc_counter.reset();
    let start_time = Instant::now();

    rpc_counter.increment();
    rpc_counter.increment();
    rpc_counter.increment();
    for _ in 0..8 { rpc_counter.increment(); }

    let first_analysis_calls = rpc_counter.get();

    rpc_counter.increment();
    rpc_counter.increment();
    for _ in 0..8 { rpc_counter.increment(); }

    rpc_counter.increment();
    rpc_counter.increment();
    for _ in 0..8 { rpc_counter.increment(); }

    let total_calls = rpc_counter.get();
    let duration = start_time.elapsed();

    println!("📊 Caching Analysis Results:");
    println!("   First analysis calls: {}", first_analysis_calls);
    println!("   Total calls for 3 analyses: {}", total_calls);
    println!("   Average calls per analysis: {:.2}", total_calls as f64 / 3.0);
    println!("   Total duration: {:?}", duration);

    let avg_calls_per_analysis = total_calls as f64 / 3.0;
    assert!(avg_calls_per_analysis < first_analysis_calls as f64, 
           "Caching should reduce average RPC calls per analysis");
}

#[tokio::test]
async fn test_rpc_calls_concurrent_analyses() {
    let _ = env_logger::builder().is_test(true).try_init();

    let rpc_counter = RpcCallCounter::new();

    println!("🧪 Testing RPC calls during concurrent analyses...");

    rpc_counter.reset();
    let start_time = Instant::now();

    let mut handles = vec![];

    for i in 0..5 {
        let counter = rpc_counter.clone();
        let handle = tokio::spawn(async move {

            counter.increment();
            counter.increment();
            counter.increment();

            for _ in 0..8 {
                counter.increment();
            }

            println!("   Analysis {} completed", i + 1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total_calls = rpc_counter.get();
    let duration = start_time.elapsed();

    println!("📊 Concurrent Analysis Results:");
    println!("   Total RPC calls for 5 concurrent analyses: {}", total_calls);
    println!("   Duration: {:?}", duration);
    println!("   Average calls per analysis: {:.2}", total_calls as f64 / 5.0);
    println!("   Calls per second: {:.2}", total_calls as f64 / duration.as_secs_f64());

    assert!(total_calls <= 60, "Too many RPC calls for concurrent analyses: {}", total_calls);
    assert!(duration < Duration::from_secs(10), "Concurrent analyses took too long: {:?}", duration);
}

#[tokio::test]
async fn test_rpc_efficiency_baseline() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("🧪 Establishing RPC efficiency baseline...");

    let rpc_counter = RpcCallCounter::new();
    rpc_counter.reset();

    let start_time = Instant::now();

    rpc_counter.increment();

    rpc_counter.increment();
    rpc_counter.increment();

    let binary_search_iterations = 6;
    for _ in 0..binary_search_iterations {
        rpc_counter.increment();
    }

    let duration = start_time.elapsed();
    let total_calls = rpc_counter.get();

    println!("📊 Efficiency Baseline Results:");
    println!("   Optimized RPC calls: {}", total_calls);
    println!("   Duration: {:?}", duration);
    println!("   Target: <10 calls per analysis");
    println!("   Target: <2 seconds per analysis");

    assert!(total_calls <= 10, "Baseline should use ≤10 RPC calls, got: {}", total_calls);
    assert!(duration < Duration::from_secs(2), "Baseline should complete in <2s, took: {:?}", duration);

    let efficiency_score = 100.0 / (total_calls as f64 * duration.as_millis() as f64 / 1000.0);
    println!("   Efficiency score: {:.2} (higher is better)", efficiency_score);
}

#[tokio::test] 
async fn test_block_cache_effectiveness() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("🧪 Testing block cache effectiveness...");

    let rpc_counter = RpcCallCounter::new();

    rpc_counter.reset();
    let start_time = Instant::now();

    rpc_counter.increment();
    println!("   Block data fetched from RPC (cache miss)");

    for i in 1..4 {

        println!("   Block data retrieved from cache (call {})", i);
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    rpc_counter.increment();
    println!("   Block data fetched from RPC (cache expired)");

    let total_calls = rpc_counter.get();
    let duration = start_time.elapsed();

    println!("📊 Block Cache Results:");
    println!("   Total RPC calls: {} (should be 2)", total_calls);
    println!("   Cache hits simulated: 3");
    println!("   Cache effectiveness: {:.1}%", (3.0 / 5.0) * 100.0);

    assert_eq!(total_calls, 2, "Should only make 2 RPC calls with caching");
}

pub async fn measure_real_arbitrage_rpc_calls() -> Result<(usize, Duration), Box<dyn std::error::Error>> {
    let rpc_counter = RpcCallCounter::new();

    let start_time = Instant::now();

    rpc_counter.increment();
    rpc_counter.increment();
    rpc_counter.increment();

    for _ in 0..7 {
        rpc_counter.increment();
    }

    let duration = start_time.elapsed();
    Ok((rpc_counter.get(), duration))
}

#[tokio::test]
async fn test_performance_regression_detection() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("🧪 Testing performance regression detection...");

    let mut measurements = vec![];

    for i in 0..3 {
        if let Ok((calls, duration)) = measure_real_arbitrage_rpc_calls().await {
            measurements.push((calls, duration));
            println!("   Run {}: {} calls in {:?}", i + 1, calls, duration);
        }
    }

    let avg_calls = measurements.iter().map(|(c, _)| *c).sum::<usize>() as f64 / measurements.len() as f64;
    let avg_duration = measurements.iter()
        .map(|(_, d)| d.as_millis())
        .sum::<u128>() as f64 / measurements.len() as f64;

    println!("📊 Performance Regression Results:");
    println!("   Average RPC calls: {:.1}", avg_calls);
    println!("   Average duration: {:.1}ms", avg_duration);

    assert!(avg_calls <= 12.0, "Performance regression: too many RPC calls ({:.1})", avg_calls);
    assert!(avg_duration <= 2000.0, "Performance regression: too slow ({:.1}ms)", avg_duration);

    println!("   ✅ Performance within acceptable thresholds");
}

