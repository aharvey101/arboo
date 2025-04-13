// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/arboo.sol";

interface IUniswapV2Pair {
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    function token0() external view returns (address);
    function token1() external view returns (address);
}


contract SimplePoolManipulation is Script {
    // Mainnet addresses
    address constant UNI = 0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984;
    address constant PEPE= 0x6982508145454Ce325dDbE47a25d4ec3d2311933;
    
    // Uniswap V3 WETH-PEPE pool
    address constant WETH_UNI_V3_POOL = 0x1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801;
    address constant WETH_PEPE_V3_PAIR = 0x11950d141EcB863F01007AdD7D1A342041227b58;
    uint24 constant FEE_TIER = 3000;
    
    // Uniswap V2
    address constant WETH_UNI_V2_PAIR = 0xd3d2E2692501A5c9Ca623199D38826e513033a17;
    address constant WETH_PEPE_V2_PAIR= 0xA43fe16908251ee70EF74718545e4FE6C5cCEc9f;
    
    // Whale address for manipulation
    address constant WHALE_ADDRESS = 0xca8711dAF13D852ED2121E4bE3894Dae366039E4;
    
    uint160 private constant MAX_SQRT_RATIO =
        1461446703485210103287273052203988822378723970342;

    function run() external {
        // Get private key for deployment
        uint256 privateKey = vm.envOr("PRIVATE_KEY", uint256(0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80));
        
        // Check initial prices
        console.log("===== INITIAL STATE =====");
        checkPrices();
        
        // Simply manipulate the price - buy a lot of PEPE with WETH
        manipulateV2Price();
        
        // Check new prices
        console.log("===== AFTER MANIPULATION =====");
        checkPrices();
        
        // Calculate suitable amount for flash swap
        uint256 flashAmount = calculateFlashAmount();
        
        // Execute the flash swap
        vm.startBroadcast(privateKey);
        
        UniswapV3FlashSwap flashSwap = new UniswapV3FlashSwap();
        console.log("FlashSwap contract deployed at:", address(flashSwap));
        
        // Fund with WETH
        IWETH(WETH).deposit{value: 0.1 ether}();
        IWETH(WETH).transfer(address(flashSwap), 0.1 ether);
        
        // Execute flash swap
        console.log("Executing flash swap with", flashAmount, "WETH");
        uint256 gasBefore = gasleft();
        
        try flashSwap.flashSwap_V3_to_V2(
            WETH_PEPE_V3_PAIR,
            FEE_TIER,
            WETH,
            PEPE,
            flashAmount
        ) {
            // In your script
            uint256 gasUsed = gasBefore - gasleft();
            console.log("Transaction successful!");
            console.log("Gas used:", gasUsed);
            
            // Check profit
            uint256 wethBalance = IERC20(WETH).balanceOf(address(flashSwap));
            console.log("WETH balance after swap:", wethBalance);
            
            // Withdraw profit
            flashSwap.withdraw();

        } catch Error(string memory reason) {
            console.log("Transaction failed with reason:", reason);
        } catch {
            console.log("Transaction failed with no reason string");
        }
        
        vm.stopBroadcast();
    }
    
    // Simple direct manipulation by buying a lot of PEPE with WETH
    function manipulateV2Price() internal {
        vm.startPrank(WHALE_ADDRESS);
        console.log("Manipulating V2 price by buying PEPE with WETH...");
        
        // Check whale WETH balance
        uint256 wethBalance = IERC20(WETH).balanceOf(WHALE_ADDRESS);
        console.log("Whale WETH balance:", wethBalance);
        
        // Use 30% of whale's WETH to buy PEPE
        uint256 wethToSwap = wethBalance * 30 / 100;
        console.log("Using", wethToSwap, "WETH to buy PEPE");
        
        // Approve router
        IERC20(WETH).approve(UNISWAP_V2_ROUTER, wethToSwap);
        
        // Execute the swap
        address[] memory path = new address[](2);
        path[0] = WETH;
        path[1] = PEPE;
        
        IUniswapV2Router02(UNISWAP_V2_ROUTER).swapExactTokensForTokens(
            wethToSwap,
            0, // No minimum output (we're not concerned with slippage here)
            path,
            WHALE_ADDRESS,
            block.timestamp
        );
        
        // Log the new reserves
        IUniswapV2Pair pair = IUniswapV2Pair(WETH_PEPE_V2_PAIR);
        (uint112 reserve0, uint112 reserve1, ) = pair.getReserves();
        address token0 = pair.token0();
        address token1 = pair.token1();
        
        console.log("New V2 reserves after manipulation:");
        console.log("  Reserve0 (", token0, "):", reserve0);
        console.log("  Reserve1 (", token1, "):", reserve1);
        
        vm.stopPrank();
    }
    
    // Check current prices on V2 and V3
    function checkPrices() internal {
        // --------- V2 Price Check ---------
        // Check V2 price (WETH to PEPE)
        address[] memory path = new address[](2);
        path[0] = WETH;
        path[1] = PEPE;
        
        uint256 testAmount = 1 ether; // 1 WETH
        uint256[] memory v2AmountsOut = IUniswapV2Router02(UNISWAP_V2_ROUTER).getAmountsOut(testAmount, path);
        
        // --------- V3 Price Check ---------
        // Query V3 pool directly using quoter contract
        uint256 v3UNIOut;
        try IQuoter(UNISWAP_V3_QUOTER).quoteExactInputSingle(
            WETH,
            PEPE,
            FEE_TIER,
            testAmount,
            MAX_SQRT_RATIO
        ) returns (uint256 amountOut) {
            v3UNIOut = amountOut;
        } catch {
            // Fallback to approximation if quoter fails
            v3UNIOut = v2AmountsOut[1] * 97 / 100;
            console.log("  V3 Quoter failed, using approximation");
        }
        
        // --------- Display Prices ---------
        console.log("Price check for 1 WETH:");
        console.log("  V2: 1 WETH -> ", v2AmountsOut[1], "PEPE");
        console.log("  V3: 1 WETH -> ", v3UNIOut, "PEPE");
        
        // Calculate price impact
        int256 priceDiffBps = int256((v2AmountsOut[1] * 10000 / v3UNIOut) - 10000);
        //console.log("  Price difference: ", priceDiffBps > 0 ? "+" : "", priceDiffBps, " bps (V2 vs V3)");
        
        // --------- Arbitrage Check ---------
        // Check V2 price (PEPEto WETH)
        path[0] = PEPE;
        path[1] = WETH;
        uint256[] memory reverseAmounts = IUniswapV2Router02(UNISWAP_V2_ROUTER).getAmountsOut(v3UNIOut, path);
        
        
        // Calculate and display profitability
//        if (reverseAmounts[1] > testAmount) {
//            uint256 profit = reverseAmounts[1] - testAmount;
//            uint256 profitPercentage = (profit * 10000) / testAmount;
//            console.log("  PROFITABLE! Profit:", profit, "WETH (", 
//                       profitPercentage / 100, ".", 
//                       profitPercentage % 100 < 10 ? "0" : "", 
//                       profitPercentage % 100, "%)");
//            
//            // Estimate gas costs
//            uint256 estimatedGasCost = 150000; // Based on your previous traces
//            uint256 gasCostInWei = estimatedGasCost * tx.gasprice;
//            console.log("  Estimated gas cost:", gasCostInWei, "wei");
//            
//            // Check if profitable after gas
//            if (profit > gasCostInWei) {
//                console.log("  Profitable after gas costs");
//            } else {
//                console.log("  NOT profitable after gas costs");
//            }
//        } else {
//            console.log("  NOT PROFITABLE. Loss:", testAmount - reverseAmounts[1], "WETH");
//        }
//        
//        // --------- Gas Estimation ---------
//        console.log("Gas estimation:");
//        console.log("  Flash swap operation: ~150,000 gas");
//        console.log("  At current gas price:", tx.gasprice, "wei");
//        console.log("  Estimated cost:", 150000 * tx.gasprice, "wei");
        }    

       // Calculate a suitable amount for the flash swap
       function calculateFlashAmount() internal view returns (uint256) {
        // Start small to ensure success
        uint256 testAmount = 0.5 ether;
        
        // Calculate more precisely if needed
        address[] memory path = new address[](2);
        path[0] = WETH;
        path[1] = PEPE;
        
        uint256[] memory v2AmountsOut = IUniswapV2Router02(UNISWAP_V2_ROUTER).getAmountsOut(testAmount, path);
        uint256 v3UNIOut = v2AmountsOut[1] * 97 / 100; // Approximation
        
        path[0] = PEPE;
        path[1] = WETH;
        uint256[] memory reverseAmounts = IUniswapV2Router02(UNISWAP_V2_ROUTER).getAmountsOut(v3UNIOut, path);
        
        if (reverseAmounts[1] > testAmount) {
            console.log("Profitable at", testAmount, "WETH. Using this amount.");
            return testAmount;
        }
        
        // If not profitable, try even smaller amount
        console.log("Not profitable at test amount. Using smaller amount for testing.");
        return 0.1 ether;
    }
}
interface IQuoter {
    function quoteExactInputSingle(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint160 sqrtPriceLimitX96
    ) external returns (uint256 amountOut);
}
// Uniswap V3 Quoter
address constant UNISWAP_V3_QUOTER = 0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6;
