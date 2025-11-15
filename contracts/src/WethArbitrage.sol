// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
}

interface IWETH {
    function deposit() external payable;
    function withdraw(uint256 wad) external;
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 value) external returns (bool);
    function approve(address spender, uint256 value) external returns (bool);
}

interface IUniswapV2Factory {
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

interface IUniswapV2Router02 {
    function factory() external pure returns (address);
    function WETH() external pure returns (address);
    
    function swapExactTokensForTokens(
        uint amountIn,
        uint amountOutMin,
        address[] calldata path,
        address to,
        uint deadline
    ) external returns (uint[] memory amounts);
    
    function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
        external
        payable
        returns (uint[] memory amounts);
        
    function swapExactTokensForETH(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline)
        external
        returns (uint[] memory amounts);
        
    function getAmountsOut(uint amountIn, address[] calldata path)
        external view returns (uint[] memory amounts);
}

interface IUniswapV2Pair {
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    function token0() external view returns (address);
    function token1() external view returns (address);
}

contract ETHArbitrageBot is ReentrancyGuard, Ownable {
    IUniswapV2Router02 public immutable uniswapRouter;
    IWETH public immutable WETH;
    IERC20 public immutable stakedETH; // Could be stETH, rETH, etc.
    
    uint256 public minProfitBasisPoints = 50; // 0.5% minimum profit
    uint256 public maxSlippageBasisPoints = 300; // 3% max slippage
    
    event ArbitrageExecuted(
        address indexed tokenA,
        address indexed tokenB,
        address indexed tokenC,
        uint256 amountIn,
        uint256 profit
    );
    
    event ProfitWithdrawn(address indexed token, uint256 amount);
    
    constructor(
        address _uniswapRouter,
        address _stakedETH
    ) {
        uniswapRouter = IUniswapV2Router02(_uniswapRouter);
        WETH = IWETH(IUniswapV2Router02(_uniswapRouter).WETH());
        stakedETH = IERC20(_stakedETH);
    }
    
    receive() external payable {}
    
    /**
     * @dev Execute triangular arbitrage between ETH, WETH, and staked ETH
     * @param amountIn Amount of starting token to use for arbitrage
     * @param path Array of token addresses for the arbitrage path
     */
    function executeArbitrage(
        uint256 amountIn,
        address[] calldata path
    ) external onlyOwner nonReentrant {
        require(path.length == 4, "Invalid path length"); // Should be [tokenA, tokenB, tokenC, tokenA]
        require(path[0] == path[3], "Path must be circular");
        
        // Validate that we're dealing with our supported tokens
        require(
            _isValidToken(path[0]) && 
            _isValidToken(path[1]) && 
            _isValidToken(path[2]),
            "Invalid token in path"
        );
        
        uint256 initialBalance = _getTokenBalance(path[0]);
        require(initialBalance >= amountIn, "Insufficient balance");
        
        // Calculate expected profit before executing
        uint256 expectedOutput = _calculateArbitrageOutput(amountIn, path);
        uint256 minProfitRequired = amountIn * minProfitBasisPoints / 10000;
        require(expectedOutput > amountIn + minProfitRequired, "Insufficient profit opportunity");
        
        // Execute the arbitrage
        _executeArbitrageSwaps(amountIn, path);
        
        uint256 finalBalance = _getTokenBalance(path[0]);
        uint256 profit = finalBalance - initialBalance + amountIn;
        
        require(profit >= minProfitRequired, "Arbitrage not profitable");
        
        emit ArbitrageExecuted(path[0], path[1], path[2], amountIn, profit);
    }
    
    /**
     * @dev Calculate potential profit from arbitrage opportunity
     * @param amountIn Starting amount
     * @param path Arbitrage path
     * @return expectedOutput Final amount after all swaps
     */
    function calculateArbitrageProfit(
        uint256 amountIn,
        address[] calldata path
    ) external view returns (uint256 expectedOutput, uint256 profit) {
        require(path.length == 4, "Invalid path length");
        require(path[0] == path[3], "Path must be circular");
        
        expectedOutput = _calculateArbitrageOutput(amountIn, path);
        profit = expectedOutput > amountIn ? expectedOutput - amountIn : 0;
    }
    
    /**
     * @dev Check if arbitrage opportunity exists
     * @param amountIn Amount to test with
     * @param path Arbitrage path
     * @return isProfitable Whether the arbitrage would be profitable
     * @return profit Expected profit amount
     */
    function checkArbitrageOpportunity(
        uint256 amountIn,
        address[] calldata path
    ) external view returns (bool isProfitable, uint256 profit) {
        if (path.length != 4 || path[0] != path[3]) {
            return (false, 0);
        }
        
        uint256 expectedOutput = _calculateArbitrageOutput(amountIn, path);
        uint256 minProfitRequired = amountIn * minProfitBasisPoints / 10000;
        
        if (expectedOutput > amountIn + minProfitRequired) {
            isProfitable = true;
            profit = expectedOutput - amountIn;
        }
    }
    
    /**
     * @dev Execute the actual swaps for arbitrage
     */
    function _executeArbitrageSwaps(uint256 amountIn, address[] calldata path) internal {
        uint256 currentAmount = amountIn;
        
        for (uint i = 0; i < path.length - 1; i++) {
            address tokenIn = path[i];
            address tokenOut = path[i + 1];
            
            // Handle ETH specially
            if (tokenIn == address(0)) {
                // ETH to token swap
                require(address(this).balance >= currentAmount, "Insufficient ETH");
                address[] memory swapPath = new address[](2);
                swapPath[0] = address(WETH);
                swapPath[1] = tokenOut;
                
                uint256[] memory amounts = uniswapRouter.swapExactETHForTokens{value: currentAmount}(
                    0, // We'll calculate minimum separately
                    swapPath,
                    address(this),
                    block.timestamp + 300
                );
                currentAmount = amounts[1];
                
            } else if (tokenOut == address(0)) {
                // Token to ETH swap
                IERC20(tokenIn).approve(address(uniswapRouter), currentAmount);
                address[] memory swapPath = new address[](2);
                swapPath[0] = tokenIn;
                swapPath[1] = address(WETH);
                
                uint256[] memory amounts = uniswapRouter.swapExactTokensForETH(
                    currentAmount,
                    0,
                    swapPath,
                    address(this),
                    block.timestamp + 300
                );
                currentAmount = amounts[1];
                
            } else {
                // Token to token swap
                IERC20(tokenIn).approve(address(uniswapRouter), currentAmount);
                address[] memory swapPath = new address[](2);
                swapPath[0] = tokenIn;
                swapPath[1] = tokenOut;
                
                uint256[] memory amounts = uniswapRouter.swapExactTokensForTokens(
                    currentAmount,
                    0,
                    swapPath,
                    address(this),
                    block.timestamp + 300
                );
                currentAmount = amounts[1];
            }
        }
    }
    
    /**
     * @dev Calculate expected output from arbitrage path
     */
    function _calculateArbitrageOutput(uint256 amountIn, address[] calldata path) internal view returns (uint256) {
        uint256 currentAmount = amountIn;
        
        for (uint i = 0; i < path.length - 1; i++) {
            address tokenIn = path[i];
            address tokenOut = path[i + 1];
            
            address[] memory swapPath = new address[](2);
            
            if (tokenIn == address(0)) {
                swapPath[0] = address(WETH);
                swapPath[1] = tokenOut;
            } else if (tokenOut == address(0)) {
                swapPath[0] = tokenIn;
                swapPath[1] = address(WETH);
            } else {
                swapPath[0] = tokenIn;
                swapPath[1] = tokenOut;
            }
            
            uint256[] memory amounts = uniswapRouter.getAmountsOut(currentAmount, swapPath);
            currentAmount = amounts[1];
        }
        
        return currentAmount;
    }
    
    /**
     * @dev Get token balance, handling ETH specially
     */
    function _getTokenBalance(address token) internal view returns (uint256) {
        if (token == address(0)) {
            return address(this).balance;
        }
        return IERC20(token).balanceOf(address(this));
    }
    
    /**
     * @dev Check if token is supported for arbitrage
     */
    function _isValidToken(address token) internal view returns (bool) {
        return token == address(0) || token == address(WETH) || token == address(stakedETH);
    }
    
    // Admin functions
    function setMinProfitBasisPoints(uint256 _minProfitBasisPoints) external onlyOwner {
        require(_minProfitBasisPoints <= 1000, "Too high"); // Max 10%
        minProfitBasisPoints = _minProfitBasisPoints;
    }
    
    function setMaxSlippageBasisPoints(uint256 _maxSlippageBasisPoints) external onlyOwner {
        require(_maxSlippageBasisPoints <= 1000, "Too high"); // Max 10%
        maxSlippageBasisPoints = _maxSlippageBasisPoints;
    }
    
    function withdrawToken(address token, uint256 amount) external onlyOwner {
        if (token == address(0)) {
            payable(owner()).transfer(amount);
        } else {
            IERC20(token).transfer(owner(), amount);
        }
        emit ProfitWithdrawn(token, amount);
    }
    
    function withdrawAllTokens() external onlyOwner {
        // Withdraw ETH
        if (address(this).balance > 0) {
            payable(owner()).transfer(address(this).balance);
        }
        
        // Withdraw WETH
        uint256 wethBalance = WETH.balanceOf(address(this));
        if (wethBalance > 0) {
            WETH.transfer(owner(), wethBalance);
        }
        
        // Withdraw staked ETH
        uint256 stakedBalance = stakedETH.balanceOf(address(this));
        if (stakedBalance > 0) {
            stakedETH.transfer(owner(), stakedBalance);
        }
    }
    
    // Emergency function to convert WETH to ETH
    function unwrapWETH(uint256 amount) external onlyOwner {
        WETH.withdraw(amount);
    }
    
    // Emergency function to wrap ETH to WETH
    function wrapETH() external payable onlyOwner {
        WETH.deposit{value: msg.value}();
    }
}
