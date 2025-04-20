// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "aave-v3-core/contracts/flashloan/base/FlashLoanReceiverBase.sol";
import "aave-v3-core/contracts/interfaces/IPoolAddressesProvider.sol";
import "aave-v3-core/contracts/interfaces/IPool.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@uniswap/v3-periphery/contracts/libraries/TransferHelper.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title UniswapV3FlashLoanArbitrage
 * @dev Smart contract that uses Aave V3 flash loans to execute arbitrage on Uniswap V3
 * Based on token flow chart
 */
contract UniswapV3FlashLoanArbitrage is FlashLoanReceiverBase, Ownable {
    // Uniswap V3 Router
    ISwapRouter public immutable swapRouter;
    
    // Token addresses
    address public constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address public constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address public constant USDT = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    
    // Contract addresses from token flow chart
    address public constant CONTRACT_0xE05 = 0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8;
    address public constant CONTRACT_0x88E = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address public constant CONTRACT_0x577 = 0x6c6Bc977E13Df9b0de53b251522280BB72383700;
    
    // Fee tiers
    uint24 public constant FEE_TIER_LOW = 500;    // 0.05%
    uint24 public constant FEE_TIER_MEDIUM = 3000; // 0.3%
    uint24 public constant FEE_TIER_HIGH = 10000;  // 1%
    
    // Deadline for swaps (10 minutes from execution)
    uint256 private constant DEADLINE = 10 minutes;
    
    // Struct to pass around arbitrage path data
    struct ArbitragePath {
        address[] tokens;
        uint24[] fees;
    }
    
    // Event emitted when arbitrage is executed
    event ArbitrageExecuted(
        address indexed initiator,
        address indexed startToken,
        uint256 startAmount,
        uint256 profit
    );
    
    /**
     * @dev Constructor
     * @param _addressProvider Aave V3 pool addresses provider
     * @param _swapRouter Uniswap V3 SwapRouter
     */
    constructor(
        IPoolAddressesProvider _addressProvider,
        ISwapRouter _swapRouter
    ) FlashLoanReceiverBase(_addressProvider) {
        swapRouter = _swapRouter;
    }
    
    /**
     * @dev Execute the arbitrage with flash loan
     * @param asset The asset to borrow from flash loan
     * @param amount The amount to borrow
     */
    function executeArbitrage(address asset, uint256 amount) external onlyOwner {
        address[] memory assets = new address[](1);
        assets[0] = asset;
        
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        
        // 0 = no debt (flash loan), 1 = stable, 2 = variable
        uint256[] memory interestRateModes = new uint256[](1);
        interestRateModes[0] = 0;
        
        // Get the pool and request flash loan
        address onBehalfOf = address(this);
        bytes memory params = abi.encode(asset, amount);
        uint16 referralCode = 0;
        
        // Use Aave V3 Pool interface
        POOL.flashLoan(
            address(this),
            assets,
            amounts,
            interestRateModes,
            onBehalfOf,
            params,
            referralCode
        );
    }
    
    /**
     * @dev This function is called after the flash loan is provided (Aave V3 format)
     * @param assets The addresses of the assets being flash-borrowed
     * @param amounts The amounts of the assets being flash-borrowed
     * @param premiums The fee to be paid for each asset
     * @param initiator The address initiating the flash loan
     * @param params Encoded parameters for the arbitrage path
     * @return boolean indicating the success of the operation
     */
    function executeOperation(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        // Verify caller is Aave Pool
        require(msg.sender == address(POOL), "Caller must be Aave Pool");
        
        // Decode params
        (address asset, uint256 amount) = abi.decode(params, (address, uint256));
        
        // Execute arbitrage based on the token flow chart
        uint256 startBalance = IERC20(asset).balanceOf(address(this));
        
        // Choose the optimal arbitrage path based on the asset
        if (asset == USDC) {
            _executeUSDCArbitragePath(amount);
        } else if (asset == WETH) {
            _executeWETHArbitragePath(amount);
        } else if (asset == DAI) {
            _executeDAIArbitragePath(amount);
        } else if (asset == USDT) {
            _executeUSDTArbitragePath(amount);
        }
        
        // Calculate profit
        uint256 endBalance = IERC20(asset).balanceOf(address(this));
        uint256 profit = endBalance > startBalance + premiums[0] 
            ? endBalance - (startBalance + premiums[0]) 
            : 0;
        
        // Approve the Pool contract to pull the flash loan amount + premium
        uint256 totalDebt = amounts[0] + premiums[0];
        IERC20(assets[0]).approve(address(POOL), totalDebt);
        
        // Emit event
        emit ArbitrageExecuted(
            initiator,
            asset,
            amount,
            profit
        );
        
        // Return true to indicate the flash loan was processed successfully
        return true;
    }
    
    /**
     * @dev Execute USDC arbitrage path based on token flow chart
     * @param amount The amount of USDC to start with
     */
    function _executeUSDCArbitragePath(uint256 amount) internal {
        // From token flow chart, we can see USDC has -73,988 flow
        // Path: USDC -> WETH -> DAI -> USDC
        
        // Approve USDC for swap
        TransferHelper.safeApprove(USDC, address(swapRouter), amount);
        
        // USDC -> WETH
        uint256 wethAmount = _swapExactInputSingle(
            USDC,
            WETH,
            FEE_TIER_MEDIUM,
            amount
        );
        
        // Approve WETH for swap
        TransferHelper.safeApprove(WETH, address(swapRouter), wethAmount);
        
        // WETH -> DAI
        uint256 daiAmount = _swapExactInputSingle(
            WETH,
            DAI,
            FEE_TIER_MEDIUM,
            wethAmount
        );
        
        // Approve DAI for swap
        TransferHelper.safeApprove(DAI, address(swapRouter), daiAmount);
        
        // DAI -> USDC
        _swapExactInputSingle(
            DAI,
            USDC,
            FEE_TIER_LOW,
            daiAmount
        );
    }
    
    /**
     * @dev Execute WETH arbitrage path
     * @param amount The amount of WETH to start with
     */
    function _executeWETHArbitragePath(uint256 amount) internal {
        // From token flow chart, WETH has -547.233 flow
        // Path: WETH -> DAI -> USDC -> WETH
        
        // Approve WETH for swap
        TransferHelper.safeApprove(WETH, address(swapRouter), amount);
        
        // WETH -> DAI
        uint256 daiAmount = _swapExactInputSingle(
            WETH,
            DAI,
            FEE_TIER_MEDIUM,
            amount
        );
        
        // Approve DAI for swap
        TransferHelper.safeApprove(DAI, address(swapRouter), daiAmount);
        
        // DAI -> USDC
        uint256 usdcAmount = _swapExactInputSingle(
            DAI,
            USDC,
            FEE_TIER_LOW,
            daiAmount
        );
        
        // Approve USDC for swap
        TransferHelper.safeApprove(USDC, address(swapRouter), usdcAmount);
        
        // USDC -> WETH
        _swapExactInputSingle(
            USDC,
            WETH,
            FEE_TIER_MEDIUM,
            usdcAmount
        );
    }
    
    /**
     * @dev Execute DAI arbitrage path
     * @param amount The amount of DAI to start with
     */
    function _executeDAIArbitragePath(uint256 amount) internal {
        // Path: DAI -> USDC -> WETH -> DAI
        
        // Approve DAI for swap
        TransferHelper.safeApprove(DAI, address(swapRouter), amount);
        
        // DAI -> USDC
        uint256 usdcAmount = _swapExactInputSingle(
            DAI,
            USDC,
            FEE_TIER_LOW,
            amount
        );
        
        // Approve USDC for swap
        TransferHelper.safeApprove(USDC, address(swapRouter), usdcAmount);
        
        // USDC -> WETH
        uint256 wethAmount = _swapExactInputSingle(
            USDC,
            WETH,
            FEE_TIER_MEDIUM,
            usdcAmount
        );
        
        // Approve WETH for swap
        TransferHelper.safeApprove(WETH, address(swapRouter), wethAmount);
        
        // WETH -> DAI
        _swapExactInputSingle(
            WETH,
            DAI,
            FEE_TIER_MEDIUM,
            wethAmount
        );
    }
    
    /**
     * @dev Execute USDT arbitrage path
     * @param amount The amount of USDT to start with
     */
    function _executeUSDTArbitragePath(uint256 amount) internal {
        // From the chart, we see negative USDT flows but positive in other paths
        // Path: USDT -> ETH -> USDC -> USDT
        
        // Approve USDT for swap
        TransferHelper.safeApprove(USDT, address(swapRouter), amount);
        
        // USDT -> ETH
        uint256 ethAmount = _swapExactInputSingle(
            USDT,
            WETH,
            FEE_TIER_MEDIUM,
            amount
        );
        
        // Approve ETH for swap
        TransferHelper.safeApprove(WETH, address(swapRouter), ethAmount);
        
        // ETH -> USDC
        uint256 usdcAmount = _swapExactInputSingle(
            WETH,
            USDC,
            FEE_TIER_MEDIUM,
            ethAmount
        );
        
        // Approve USDC for swap
        TransferHelper.safeApprove(USDC, address(swapRouter), usdcAmount);
        
        // USDC -> USDT
        _swapExactInputSingle(
            USDC,
            USDT,
            FEE_TIER_LOW,
            usdcAmount
        );
    }
    
    /**
     * @dev Performs an exact input single swap on Uniswap V3
     * @param tokenIn The token to swap from
     * @param tokenOut The token to swap to
     * @param fee The fee tier to use
     * @param amountIn The amount of tokenIn to swap
     * @return amountOut The amount of tokenOut received
     */
    function _swapExactInputSingle(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn
    ) internal returns (uint256 amountOut) {
        // Create the parameters for the swap
        ISwapRouter.ExactInputSingleParams memory params = ISwapRouter.ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: fee,
            recipient: address(this),
            deadline: block.timestamp + DEADLINE,
            amountIn: amountIn,
            amountOutMinimum: 0, // In production, calculate this with oracle
            sqrtPriceLimitX96: 0
        });
        
        // Execute the swap
        amountOut = swapRouter.exactInputSingle(params);
    }
    
    /**
     * @dev Multiple token swap following a specific path on Uniswap V3
     * @param path Encoded swap path
     * @param amountIn Amount of first token to swap
     * @return amountOut Amount of last token received
     */
    function _swapExactInputMulti(
        bytes memory path,
        uint256 amountIn
    ) internal returns (uint256 amountOut) {
        // Create the parameters for the swap
        ISwapRouter.ExactInputParams memory params = ISwapRouter.ExactInputParams({
            path: path,
            recipient: address(this),
            deadline: block.timestamp + DEADLINE,
            amountIn: amountIn,
            amountOutMinimum: 0 // In production, calculate this with oracle
        });
        
        // Execute the swap
        amountOut = swapRouter.exactInput(params);
    }
    
    /**
     * @dev Withdraw profits to owner
     * @param token Token to withdraw
     */
    function withdrawProfit(address token) external onlyOwner {
        uint256 balance = IERC20(token).balanceOf(address(this));
        if (balance > 0) {
            TransferHelper.safeTransfer(token, owner(), balance);
        }
    }
    
    /**
     * @dev Withdraw all profits to owner
     */
    function withdrawAllProfits() external onlyOwner {
        // Withdraw USDC
        uint256 usdcBalance = IERC20(USDC).balanceOf(address(this));
        if (usdcBalance > 0) {
            TransferHelper.safeTransfer(USDC, owner(), usdcBalance);
        }
        
        // Withdraw WETH
        uint256 wethBalance = IERC20(WETH).balanceOf(address(this));
        if (wethBalance > 0) {
            TransferHelper.safeTransfer(WETH, owner(), wethBalance);
        }
        
        // Withdraw DAI
        uint256 daiBalance = IERC20(DAI).balanceOf(address(this));
        if (daiBalance > 0) {
            TransferHelper.safeTransfer(DAI, owner(), daiBalance);
        }
        
        // Withdraw USDT
        uint256 usdtBalance = IERC20(USDT).balanceOf(address(this));
        if (usdtBalance > 0) {
            TransferHelper.safeTransfer(USDT, owner(), usdtBalance);
        }
    }
    
    /**
     * @dev Receive ETH
     */
    receive() external payable {}
}
