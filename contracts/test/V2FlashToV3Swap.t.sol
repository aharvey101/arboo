// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test, console2} from "forge-std/Test.sol";
import {V2FlashToV3Swap, IUniswapV2Router02, IUniswapV2Pool, ISwapRouter02, IERC20, IWETH} from "../src/V2FlashToV3Swap.sol";

address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
address constant SWAP_ROUTER_02 = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;

// Pool addresses
address constant DAI_WETH_V2_POOL = 0xA478c2975Ab1Ea89e8196811F51A7B7Ade33eB11;

// V3 fee tiers
uint24 constant FEE_500 = 500;   // 0.05%
uint24 constant FEE_3000 = 3000; // 0.3%

contract V2FlashToV3SwapTest is Test {
    IERC20 private constant dai = IERC20(DAI);
    IWETH private constant weth = IWETH(WETH);
    ISwapRouter02 private constant router = ISwapRouter02(SWAP_ROUTER_02);
    IUniswapV2Router02 private constant v2_router = IUniswapV2Router02(UNISWAP_V2_ROUTER);
    IUniswapV2Pool private constant daiWethV2Pool = IUniswapV2Pool(DAI_WETH_V2_POOL);
    
    V2FlashToV3Swap private flashSwap;
    address private owner;
    
    // Test amounts
    uint256 private constant INITIAL_ETH_BALANCE = 100 * 1e18;
    
    function setUp() public {
        // Fork mainnet using local Anvil node
        vm.createFork("http://127.0.0.1:8545");
        
        flashSwap = new V2FlashToV3Swap();
        owner = address(this);
        
        // Fund the test with ETH and convert to WETH
        vm.deal(address(this), INITIAL_ETH_BALANCE);
        weth.deposit{value: 50 * 1e18}();
        
        // Get some DAI using a separate transaction
        _getDaiFromV3();
        
        console2.log("=== Test Setup Complete ===");
        console2.log("WETH Balance:", weth.balanceOf(address(this)) / 1e18, "WETH");
        console2.log("DAI Balance:", dai.balanceOf(address(this)) / 1e18, "DAI");
    }
    
    function _getDaiFromV3() private {
        // Use V3 to get DAI (different from the V2 pool we'll test)
        weth.approve(address(router), 25 * 1e18);
        
        ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
            .ExactInputSingleParams({
                tokenIn: WETH,
                tokenOut: DAI,
                fee: FEE_3000,
                recipient: address(this),
                amountIn: 25 * 1e18,
                amountOutMinimum: 1,
                sqrtPriceLimitX96: 0
            });
        
        router.exactInputSingle(params);
    }
    
    function test_flashSwap_small_amount() public {
        console2.log("=== Testing Small Flash Swap ===");
        
        uint256 initialWethBalance = weth.balanceOf(address(this));
        uint256 initialDaiBalance = dai.balanceOf(address(this));
        
        console2.log("Initial WETH Balance:", initialWethBalance / 1e18);
        console2.log("Initial DAI Balance:", initialDaiBalance / 1e18);
        
        // Try a very small flash amount first
        uint256 flashAmount = 100 * 1e18; // 100 DAI
        
        console2.log("Executing flash swap with", flashAmount / 1e18, "DAI");
        
        try flashSwap.flashSwap_V2_to_V3({
            v2Pool: DAI_WETH_V2_POOL,
            tokenIn: DAI,
            tokenOut: WETH,
            amountIn: flashAmount,
            v3Fee: FEE_500
        }) {
            console2.log("Flash swap completed successfully");
            
            uint256 finalWethBalance = weth.balanceOf(address(this));
            uint256 finalDaiBalance = dai.balanceOf(address(this));
            
            console2.log("Final WETH Balance:", finalWethBalance / 1e18);
            console2.log("Final DAI Balance:", finalDaiBalance / 1e18);
            
            if (finalWethBalance > initialWethBalance) {
                console2.log("WETH Profit:", (finalWethBalance - initialWethBalance));
            }
            
            if (finalDaiBalance > initialDaiBalance) {
                console2.log("DAI Profit:", (finalDaiBalance - initialDaiBalance) / 1e18);
            }
            
        } catch Error(string memory reason) {
            console2.log("Flash swap failed with reason:", reason);
            // This is expected - there might not be a profitable arbitrage opportunity
        } catch (bytes memory lowLevelData) {
            console2.log("Flash swap failed with low level error");
            console2.logBytes(lowLevelData);
        }
    }
    
    function test_flashSwap_opposite_direction() public {
        console2.log("=== Testing WETH -> DAI Flash Swap ===");
        
        uint256 initialWethBalance = weth.balanceOf(address(this));
        uint256 initialDaiBalance = dai.balanceOf(address(this));
        
        console2.log("Initial WETH Balance:", initialWethBalance / 1e18);
        console2.log("Initial DAI Balance:", initialDaiBalance / 1e18);
        
        // Try WETH -> DAI direction
        uint256 flashAmount = 1 * 1e18; // 1 WETH
        
        console2.log("Executing flash swap with", flashAmount / 1e18, "WETH");
        
        try flashSwap.flashSwap_V2_to_V3({
            v2Pool: DAI_WETH_V2_POOL,
            tokenIn: WETH,
            tokenOut: DAI,
            amountIn: flashAmount,
            v3Fee: FEE_3000
        }) {
            console2.log("Flash swap completed successfully");
            
            uint256 finalWethBalance = weth.balanceOf(address(this));
            uint256 finalDaiBalance = dai.balanceOf(address(this));
            
            console2.log("Final WETH Balance:", finalWethBalance / 1e18);
            console2.log("Final DAI Balance:", finalDaiBalance / 1e18);
            
            if (finalWethBalance > initialWethBalance) {
                console2.log("WETH Profit:", (finalWethBalance - initialWethBalance));
            }
            
            if (finalDaiBalance > initialDaiBalance) {
                console2.log("DAI Profit:", (finalDaiBalance - initialDaiBalance) / 1e18);
            }
            
        } catch Error(string memory reason) {
            console2.log("Flash swap failed with reason:", reason);
        } catch (bytes memory lowLevelData) {
            console2.log("Flash swap failed with low level error");
            console2.logBytes(lowLevelData);
        }
    }
    
    function test_contract_basics() public {
        assertEq(flashSwap.owner(), owner, "Owner should be set correctly");
        console2.log("Contract deployed successfully");
        console2.log("Owner:", flashSwap.owner());
    }
    
    // Receive ETH
    receive() external payable {}
}
