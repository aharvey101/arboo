# E2E Testing Documentation

## Overview
This directory contains comprehensive end-to-end tests for the Arboo arbitrage bot. These tests verify the complete functionality of the system from blockchain interaction to arbitrage execution, including integration tests that validate component interactions and full system workflows.

## Structure

```
tests/
├── bin/
│   └── e2e_test_runner.rs    # Standalone test runner binary with full integration tests
├── utils/
│   ├── mod.rs                # Utilities module
│   ├── test_env.rs           # Basic test environment setup
│   ├── integrated_test_env.rs # Advanced integration test environment
│   ├── anvil_setup.rs        # Local blockchain fork management
│   ├── contract_deployment.rs # Test contract deployment utilities
│   ├── mock_websocket.rs     # Mock WebSocket provider for testing
│   └── pool_test_runner.rs   # Pool testing utilities
├── fixtures/
│   ├── mod.rs                # Test fixtures module
│   ├── test_scenarios.rs     # Predefined test scenarios
│   └── contracts/            # Test contract artifacts
├── atomic_tests.rs           # Basic functionality tests
├── *.rs                      # Various specialized test modules
├── .env.test.example         # Test configuration template
└── README.md                 # This file
```

## Running Tests

### Method 1: Using the Test Runner Binary (Recommended)
```bash
# Run all tests (atomic + integration)
cargo run --bin e2e_test_runner all

# Run specific test categories
cargo run --bin e2e_test_runner atomic      # Basic functionality tests
cargo run --bin e2e_test_runner integration # Full integration test suite

# Run with enhanced logging
RUST_LOG=debug cargo run --bin e2e_test_runner all
RUST_LOG=info cargo run --bin e2e_test_runner integration

# Run with custom environment
TEST_WS_URL=wss://your-node.com cargo run --bin e2e_test_runner all
```

### Method 2: Using Cargo Test
```bash
# Run atomic tests
cargo test --test atomic_tests

# Run specific test modules
cargo test --test arbitrage_calculation_tests
cargo test --test pool_data_tests

# Run all E2E tests
cargo test --test "*"

# Run with logging
RUST_LOG=info cargo test --test atomic_tests
```

## Test Categories

### 1. Atomic Tests ✅ (Implemented)
- **Provider Connection**: Basic blockchain connectivity validation
- **Provider Stability**: Connection resilience under load and timeout scenarios
- **Block Data Integrity**: Validation of blockchain data consistency and format
- **Basic Environment Setup**: Test environment initialization and cleanup

### 2. Integration Tests ✅ (Implemented)
- **End-to-End Arbitrage Pipeline**: Complete arbitrage detection and execution flow
- **Pool-Strategy Integration**: Validation of pool data management with arbitrage strategies
- **EVM-Pool State Integration**: EVM simulation with real-time pool state synchronization
- **Provider Pipeline Integration**: HTTP/WebSocket provider data pipeline with concurrent queries
- **Strategy Processing Integration**: Arbitrage strategy processing with realistic market scenarios
- **Multi-Component System Integration**: Full system integration across all major components

### 3. Component Tests ✅ (Implemented)
- **Pool Data Loading**: Pool discovery, caching, and data integrity validation
- **EVM Simulator**: Local blockchain simulation with Anvil integration
- **Transaction Creation**: Transaction building, gas estimation, and simulation
- **Mock Provider Integration**: WebSocket provider mocking and scenario simulation

### 4. Specialized Test Modules ✅ (Available)
- **Arbitrage Calculation Tests**: Profit calculation accuracy and edge cases
- **Pool Pairing Tests**: Pool relationship mapping and arbitrage path discovery  
- **Gas Price Handling**: Dynamic gas pricing and spike scenario testing
- **Error Recovery**: Network disconnection and transaction failure recovery
- **Memory Profiling**: Memory usage analysis under high-frequency operations
- **Benchmark Tests**: Performance testing for opportunity detection and execution

## Configuration

### Basic Setup
1. Copy the test environment template:
   ```bash
   cp tests/.env.test.example tests/.env.test
   ```

2. Edit `tests/.env.test` with your test configuration:
   - Use a reliable WebSocket endpoint
   - Set appropriate timeouts for your network
   - Configure local fork settings if needed

### Advanced Configuration
The integration tests support additional environment variables:
- `TEST_FORK_BLOCK_NUMBER`: Specific block number for forking (optional)
- `TEST_ENABLE_ANVIL`: Enable local Anvil blockchain for testing (default: true)
- `TEST_GAS_LIMIT`: Custom gas limit for test transactions
- `TEST_TIMEOUT_SECS`: Global test timeout in seconds

## Test Environment Options

### Public Endpoints (Recommended for CI)
- Ethereum Mainnet: `wss://eth.merkle.io`
- Polygon: `wss://polygon.merkle.io` 
- Arbitrum: `wss://arbitrum.merkle.io`
- Base: `wss://base.merkle.io`

### Local Development (Recommended for Integration Tests)
- Anvil Fork: `ws://127.0.0.1:8545` (automatically managed)
- Hardhat: `ws://127.0.0.1:8545`
- Ganache: `ws://127.0.0.1:7545`

### Testing Infrastructure
The integration tests automatically set up:
- Local Anvil blockchain fork from mainnet
- Mock WebSocket providers for scenario testing
- Test token and pool contract deployments
- Realistic arbitrage scenarios with actual market data

## Adding New Tests

### 1. Atomic Tests
Add to `atomic_tests.rs` for basic functionality verification:
```rust
#[tokio::test]
async fn test_new_atomic_feature() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    // Your basic test logic here
    Ok(())
}
```

### 2. Integration Tests
Add new integration tests to `bin/e2e_test_runner.rs`:
```rust
async fn test_my_new_integration() -> Result<()> {
    info!("🧪 Testing new integration feature");
    
    let test_env = IntegratedTestEnvironment::new().await?;
    let anvil = test_env.anvil();
    
    // Your complex integration test logic here
    // - Multi-component interaction testing
    // - Real blockchain state validation
    // - End-to-end workflow verification
    
    Ok(())
}
```

### 3. Specialized Test Modules
Create new test files for specific functionality:
```rust
// tests/my_feature_tests.rs
use crate::utils::{IntegratedTestEnvironment, AnvilInstance};
use anyhow::Result;

#[tokio::test]
async fn test_specific_feature() -> Result<()> {
    // Focused test for specific feature
    Ok(())
}
```

### 4. Performance Tests
Add benchmarks to existing benchmark files:
```rust
// tests/performance_benchmarks.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_new_feature(c: &mut Criterion) {
    c.bench_function("new_feature", |b| {
        b.iter(|| {
            // Benchmark logic
        })
    });
}
```

## Best Practices

1. **Test Isolation**: Each test should be completely independent and not rely on state from other tests
2. **Resource Cleanup**: Always clean up blockchain state, file system changes, and network connections
3. **Realistic Testing**: Use actual mainnet data and realistic scenarios in integration tests
4. **Comprehensive Timeouts**: Set reasonable timeouts for all network operations and async tasks
5. **Descriptive Assertions**: Include helpful error messages that aid in debugging test failures
6. **Structured Logging**: Use appropriate log levels (debug/info/warn/error) with descriptive messages
7. **Error Handling**: Test both success and failure scenarios comprehensively
8. **Performance Awareness**: Monitor test execution time and resource usage

## Integration Test Architecture

The integration tests follow a layered architecture:

```
┌─────────────────────────────────────┐
│     Integration Test Runner        │
│  (bin/e2e_test_runner.rs)          │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│    Test Environment Layer          │
│  - IntegratedTestEnvironment        │
│  - AnvilInstance                    │
│  - MockWebSocketProvider            │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│     Component Layer                 │
│  - Pool Management                  │
│  - EVM Simulation                   │
│  - Strategy Processing              │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│    Infrastructure Layer             │
│  - Blockchain Providers             │
│  - Contract Deployment              │
│  - Data Pipeline                    │
└─────────────────────────────────────┘
```

## Troubleshooting

### Common Issues

1. **Integration Test Failures**
   - Verify Anvil is installed: `foundry --version`
   - Check if port 8545 is available
   - Ensure sufficient disk space for blockchain fork
   - Validate mainnet RPC endpoint is accessible

2. **Connection Timeouts**
   - Check your internet connection stability
   - Try a different WebSocket endpoint from the recommended list
   - Increase `TEST_TIMEOUT_SECS` in your environment
   - Consider using authenticated endpoints for better reliability

3. **Block Data Validation Failures**
   - Ensure you're connecting to a synced mainnet node
   - Check if the endpoint supports the required block range
   - Verify timestamp assertions account for network delays
   - Try testing with a more recent block number

4. **Rate Limiting and Performance Issues**
   - Use authenticated endpoints when available
   - Add delays between rapid API calls in custom tests
   - Consider using local Anvil fork for heavy testing scenarios
   - Monitor your endpoint's rate limits and usage

5. **Memory and Resource Issues**
   - Close other applications consuming significant memory
   - Increase system swap space if running memory-intensive tests
   - Use `cargo test --release` for better performance in CI
   - Consider running tests in smaller batches

### Debugging Steps

1. **Enable Debug Logging**
   ```bash
   RUST_LOG=debug cargo run --bin e2e_test_runner all
   ```

2. **Check Individual Components**
   ```bash
   # Test only atomic functionality
   cargo run --bin e2e_test_runner atomic
   
   # Test specific integration area
   RUST_LOG=info cargo test --test pool_data_tests
   ```

3. **Verify Environment Setup**
   - Check your `.env.test` configuration
   - Test with a simple public endpoint first
   - Validate network connectivity and firewall settings
   - Ensure all required dependencies are installed

4. **Anvil-Specific Issues**
   ```bash
   # Check Anvil installation
   anvil --version
   
   # Test manual Anvil startup
   anvil --fork-url https://eth.merkle.io --port 8545
   ```

### Performance Monitoring

The integration tests include built-in performance monitoring:
- Execution time tracking for each test phase
- Memory usage profiling for intensive operations  
- Network request timing and retry statistics
- Gas usage analysis for transaction simulations

### Getting Help

- **Log Analysis**: Always check logs with `RUST_LOG=debug` for detailed execution traces
- **Environment Validation**: Verify your test environment configuration matches examples
- **Incremental Testing**: Start with atomic tests, then progress to integration tests
- **Community Support**: Check GitHub issues for similar problems and solutions
- **Documentation**: Refer to individual test module documentation for specific requirements
