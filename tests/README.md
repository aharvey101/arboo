# E2E Testing Documentation

## Overview
This directory contains end-to-end tests for the Arboo arbitrage bot. These tests verify the complete functionality of the system from blockchain interaction to arbitrage execution.

## Structure

```
tests/
├── bin/
│   └── e2e_test_runner.rs    # Standalone test runner binary
├── utils/
│   ├── mod.rs                # Utilities module
│   └── test_env.rs           # Test environment setup
├── fixtures/                 # Test data and scenarios
├── atomic_tests.rs           # Most basic tests
├── .env.test.example         # Test configuration template
└── README.md                 # This file
```

## Running Tests

### Method 1: Using the Test Runner Binary
```bash
# Run all tests
cargo run --bin e2e_test_runner

# Run specific test category
cargo run --bin e2e_test_runner provider
cargo run --bin e2e_test_runner atomic
cargo run --bin e2e_test_runner integration

# Run with custom environment
TEST_WS_URL=wss://your-node.com cargo run --bin e2e_test_runner
```

### Method 2: Using Cargo Test
```bash
# Run atomic tests
cargo test --test atomic_tests

# Run all E2E tests
cargo test --test "*"

# Run with logging
RUST_LOG=info cargo test --test atomic_tests
```

## Test Categories

### 1. Atomic Tests (Current Implementation)
- **Provider Connection**: Basic blockchain connectivity
- **Provider Stability**: Connection resilience under load  
- **Block Data Integrity**: Validation of blockchain data

### 2. Component Tests (Future)
- Pool data loading and caching
- EVM simulator functionality
- Transaction creation and simulation

### 3. Integration Tests (Future)
- Log event processing
- Arbitrage opportunity detection
- Profit calculation accuracy

### 4. Full Flow Tests (Future)
- Complete arbitrage execution cycle
- Error recovery scenarios
- Performance under load

## Configuration

1. Copy the test environment template:
   ```bash
   cp tests/.env.test.example tests/.env.test
   ```

2. Edit `tests/.env.test` with your test configuration:
   - Use a reliable WebSocket endpoint
   - Set appropriate timeouts
   - Configure local fork settings if needed

## Test Environment Options

### Public Endpoints (Recommended for CI)
- Ethereum Mainnet: `wss://eth.merkle.io`
- Polygon: `wss://polygon.merkle.io`
- Arbitrum: `wss://arbitrum.merkle.io`

### Local Development
- Anvil Fork: `ws://127.0.0.1:8545`
- Hardhat: `ws://127.0.0.1:8545`
- Ganache: `ws://127.0.0.1:7545`

## Adding New Tests

### 1. Atomic Tests
Add to `atomic_tests.rs` for basic functionality verification:
```rust
#[tokio::test]
async fn test_new_atomic_feature() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    // Your test logic here
    Ok(())
}
```

### 2. Integration Tests
Create new files like `integration_tests.rs` for complex scenarios:
```rust
use crate::utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_arbitrage_detection() -> Result<()> {
    // Complex test logic
}
```

### 3. Performance Tests
Use criterion for benchmarking:
```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_arbitrage_calculation(c: &mut Criterion) {
    // Benchmark logic
}
```

## Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Clean up any state changes after tests
3. **Timeouts**: Always set reasonable timeouts
4. **Assertions**: Use descriptive assertion messages
5. **Logging**: Include helpful debug information

## Troubleshooting

### Common Issues

1. **Connection Timeouts**
   - Check your internet connection
   - Try a different WebSocket endpoint
   - Increase `TEST_TIMEOUT_SECS`

2. **Block Data Validation Failures**
   - Ensure you're connecting to mainnet
   - Check if the endpoint is synced
   - Verify timestamp assertions are reasonable

3. **Rate Limiting**
   - Use authenticated endpoints if available
   - Add delays between rapid calls
   - Consider using local fork for heavy testing

### Getting Help

- Check the logs with `RUST_LOG=debug`
- Verify your `.env.test` configuration
- Test with a simple public endpoint first
- Check network connectivity and firewall settings
