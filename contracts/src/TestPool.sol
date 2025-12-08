// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// Simple test ERC20 token for testing
contract TestToken is ERC20 {
    uint8 private _decimals;

    constructor(
        string memory name,
        string memory symbol,
        uint8 decimals_,
        uint256 initialSupply
    ) ERC20(name, symbol) {
        _decimals = decimals_;
        _mint(msg.sender, initialSupply);
    }

    function decimals() public view override returns (uint8) {
        return _decimals;
    }
}

/// Mock Uniswap V2-like pool for testing arbitrage
contract TestPoolV2 {
    IERC20 public token0;
    IERC20 public token1;
    
    uint256 public reserve0;
    uint256 public reserve1;
    uint256 public price; // price of token1 in terms of token0 (scaled by 1e18)
    
    address public owner;

    event Swap(address indexed sender, uint256 amount0In, uint256 amount1Out);
    event Mint(address indexed sender, uint256 amount0, uint256 amount1);

    constructor(address _token0, address _token1) {
        token0 = IERC20(_token0);
        token1 = IERC20(_token1);
        owner = msg.sender;
        reserve0 = 0;
        reserve1 = 0;
        price = 1e18; // 1:1 price initially
    }

    /// Initialize pool with liquidity and price
    function initialize(
        uint256 amount0,
        uint256 amount1,
        uint256 initialPrice
    ) external {
        require(msg.sender == owner, "Only owner");
        require(amount0 > 0 && amount1 > 0, "Invalid amounts");

        // Transfer tokens to pool
        require(token0.transferFrom(msg.sender, address(this), amount0), "Token0 transfer failed");
        require(token1.transferFrom(msg.sender, address(this), amount1), "Token1 transfer failed");

        reserve0 = amount0;
        reserve1 = amount1;
        price = (amount1 * 1e18) / amount0;

        emit Mint(msg.sender, amount0, amount1);
    }

    /// Swap token0 for token1 (simple constant product formula)
    function swap(uint256 amountIn) external returns (uint256 amountOut) {
        require(amountIn > 0, "Invalid amount");
        require(token0.transferFrom(msg.sender, address(this), amountIn), "Transfer failed");

        // Calculate output using constant product formula: x * y = k
        uint256 k = reserve0 * reserve1;
        uint256 newReserve0 = reserve0 + amountIn;
        uint256 newReserve1 = k / newReserve0;
        amountOut = reserve1 - newReserve1;

        require(amountOut > 0, "Insufficient output");

        // Update reserves
        reserve0 = newReserve0;
        reserve1 = newReserve1;

        // Update price
        price = (reserve1 * 1e18) / reserve0;

        // Send output tokens
        require(token1.transfer(msg.sender, amountOut), "Transfer failed");

        emit Swap(msg.sender, amountIn, amountOut);
    }

    /// Set price manually (for testing)
    function setPrice(uint256 newPrice) external {
        require(msg.sender == owner, "Only owner");
        price = newPrice;
    }

    /// Get price (token1 per token0)
    function getPrice() external view returns (uint256) {
        if (reserve0 == 0) return 0;
        return (reserve1 * 1e18) / reserve0;
    }

    /// Get reserves
    function getReserves() external view returns (uint256 _reserve0, uint256 _reserve1) {
        return (reserve0, reserve1);
    }
}

/// Mock Uniswap V3-like pool for testing arbitrage
contract TestPoolV3 {
    IERC20 public token0;
    IERC20 public token1;
    uint24 public fee;
    
    uint256 public liquidity;
    uint256 public sqrtPriceX96; // Price stored as sqrt(price) * 2^96
    
    address public owner;

    event Swap(address indexed sender, int256 amount0, int256 amount1);
    event Mint(address indexed sender, uint256 amount0, uint256 amount1);

    constructor(
        address _token0,
        address _token1,
        uint24 _fee
    ) {
        token0 = IERC20(_token0);
        token1 = IERC20(_token1);
        fee = _fee;
        owner = msg.sender;
        liquidity = 0;
        sqrtPriceX96 = 2**96; // Price of 1 initially
    }

    /// Initialize pool with liquidity and price
    function initialize(
        uint256 initialSqrtPriceX96
    ) external {
        require(msg.sender == owner, "Only owner");
        require(initialSqrtPriceX96 > 0, "Invalid price");
        sqrtPriceX96 = initialSqrtPriceX96;
    }

    /// Mint liquidity
    function mint(
        address recipient,
        int24 tickLower,
        int24 tickUpper,
        uint128 amount,
        bytes calldata data
    ) external returns (uint256 amount0, uint256 amount1) {
        // Simplified: just track liquidity
        liquidity += amount;

        // Estimate amounts based on current price
        amount0 = (amount * 1e18) / (sqrtPriceX96 / (2**48));
        amount1 = amount;

        emit Mint(recipient, amount0, amount1);
    }

    /// Swap tokens
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1) {
        uint256 absAmount = uint256(amountSpecified > 0 ? amountSpecified : -amountSpecified);

        if (zeroForOne) {
            // Swap token0 for token1
            require(token0.transferFrom(msg.sender, address(this), absAmount), "Transfer failed");

            // Calculate output based on price
            int256 amountOut = int256((absAmount * sqrtPriceX96) / (2**96));
            amount0 = -int256(absAmount);
            amount1 = amountOut;

            // Update price slightly (simplified)
            sqrtPriceX96 = uint160(uint256(int256(sqrtPriceX96) - int256(sqrtPriceX96) / 100));

            require(token1.transfer(recipient, uint256(amountOut)), "Transfer failed");
        } else {
            // Swap token1 for token0
            require(token1.transferFrom(msg.sender, address(this), absAmount), "Transfer failed");

            // Calculate output based on price
            int256 amountOut = int256((absAmount * (2**96)) / sqrtPriceX96);
            amount1 = -int256(absAmount);
            amount0 = amountOut;

            // Update price slightly (simplified)
            sqrtPriceX96 = uint160(uint256(int256(sqrtPriceX96) + int256(sqrtPriceX96) / 100));

            require(token0.transfer(recipient, uint256(amountOut)), "Transfer failed");
        }

        emit Swap(msg.sender, amount0, amount1);
    }

    /// Set price manually (for testing)
    function setPrice(uint160 newSqrtPriceX96) external {
        require(msg.sender == owner, "Only owner");
        sqrtPriceX96 = newSqrtPriceX96;
    }

    /// Get current price
    function getPrice() external view returns (uint160) {
        return uint160(sqrtPriceX96);
    }
}
