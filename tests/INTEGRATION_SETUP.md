# Test Environment Setup

## Running Integration Tests

The integration tests require a proper test environment setup to work correctly. The tests use Anvil (from Foundry) to create a local fork of mainnet for realistic testing.

### Required Environment Variables

For the integration tests to pass, you need to set up the following environment variables:

```bash
# Required: Mainnet RPC URL for forking
MAINNET_RPC_URL=https://eth.llamarpc.com

# Optional: Specific block number to fork from (uses latest if not set)
FORK_BLOCK_NUMBER=19000000
```

### Quick Setup

1. Copy the test configuration template:
   ```bash
   cp tests/.envtestexample tests/.env.test
   ```

2. Edit `tests/.env.test` and set your `MAINNET_RPC_URL`. You can use:
   - LlamaRPC (free): `https://eth.llamarpc.com`
   - Alchemy: Get API key from https://dashboard.alchemy.com/
   - Infura: Get API key from https://infura.io/
   - QuickNode: Get API key from https://www.quicknode.com/

3. Run the tests:
   ```bash
   # Run specific test
   cargo test integration::full_arbitrage_cycle_tests::test_complete_arbitrage_cycle

   # Run all integration tests
   cargo test integration
   ```

### What the Test Environment Does

1. **Loads Configuration**: Automatically loads environment variables from `tests/.env.test`
2. **Creates Anvil Fork**: Starts a local Anvil instance that forks from mainnet at the specified block
3. **Provides Realistic Environment**: Uses real mainnet state for accurate arbitrage simulation
4. **Cleans Up**: Automatically stops Anvil instances when tests complete

### Troubleshooting

If tests fail with connection errors:

1. **Missing MAINNET_RPC_URL**: The test will create a clean Anvil instance instead of a mainnet fork
2. **Network Issues**: Try a different RPC provider
3. **Anvil Not Found**: Install Foundry: `curl -L https://foundry.paradigm.xyz | bash && foundryup`

### Recent Fixes

- Fixed environment variable loading in test setup
- Added proper mainnet forking with block number support  
- Fixed WebSocket URL handling in arbitrage strategy processing
- Added fallback behavior when MAINNET_RPC_URL is not available
