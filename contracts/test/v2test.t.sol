// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test, console2} from "forge-std/Test.sol";
import {UniswapV2FlashSwap, IUniswapV2Router02, IUniswapV3Pool, ISwapRouter02, IERC20, IWETH} from "../src/UniswapV2FlashSwap.sol";

address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
address constant SWAP_ROUTER_02 = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
address constant DAI_WETH_V2_POOL = 0xA478c2975Ab1Ea89e8196811F51A7B7Ade33eB11;
address constant DAI_WETH_POOL_3000 = 0xC2e9F25Be6257c210d7Adf0D4Cd6E3E881ba25f8;
address constant DAI_WETH_POOL_500 = 0x60594a405d53811d3BC4766596EFD80fd545A270;

uint24 constant FEE_0 = 3000;
uint24 constant FEE_1 = 500;

contract UniswapV2FlashTest is Test {
    IERC20 private constant dai = IERC20(DAI);
    IWETH private constant weth = IWETH(WETH);
    ISwapRouter02 private constant router = ISwapRouter02(SWAP_ROUTER_02);
    IUniswapV2Router02 constant v2_router =
        IUniswapV2Router02(UNISWAP_V2_ROUTER);
    UniswapV2FlashSwap private flashSwap;

    // Test amounts
    uint256 private constant WETH_AMOUNT_IN = 9000 * 1e18;
    uint256 private constant DAI_INITIAL_BALANCE = 10000000 * 1e18;
    uint256 private constant V3_DAI_SWAP_AMOUNT = 99000 * 1e18;
    uint256 private constant INITIAL_WETH_BALANCE = 10000 * 1e18;
    uint256 private constant V3_SWAP_AMOUNT = 8000 * 1e18;

    function setUp() public {
        flashSwap = new UniswapV2FlashSwap();

        // Get initial tokens
        deal(address(dai), address(this), DAI_INITIAL_BALANCE);

        // Create price discrepancy:
        // 1. First push WETH price DOWN on V2
        dai.approve(address(v2_router), type(uint256).max);

        address[] memory path = new address[](2);
        path[0] = DAI;
        path[1] = WETH;

        // Large swap on V2 to push WETH price down
        v2_router.swapExactTokensForTokens(
            100_000 * 1e18, // 5M DAI
            0,
            path,
            address(this),
            block.timestamp
        );

        weth.approve(address(router), type(uint256).max);

    }

    function test_flashSwap_V2_to_V3() public {
        uint256 balanceBefore = weth.balanceOf(address(this));

        // Try with smaller amount first
        uint256 flashAmount = 1 * 1e18; // 10 WETH

        flashSwap.flashSwap_V2_to_V3({
            pool0: DAI_WETH_V2_POOL,
            fee1: FEE_1,
            tokenIn: DAI,
            tokenOut: WETH,
            amountIn: flashAmount
        });

        uint256 balanceAfter = weth.balanceOf(address(this));
        console2.log("balance After", balanceAfter);
        uint256 profit = balanceAfter - balanceBefore;

        assertGt(profit, 0, "No profit generated");
        console2.log("Profit in WETH: %e", profit);
    }

    function test_flashSwap_V2_to_V3_different_amounts() public {
        uint256[] memory testAmounts = new uint256[](3);
        testAmounts[0] = 10 * 1e18; // 10 WETH
        testAmounts[1] = 20 * 1e18; // 20 WETH
        testAmounts[2] = 30 * 1e18; // 30 WETH

        for (uint i = 0; i < testAmounts.length; i++) {
            uint256 balanceBefore = weth.balanceOf(address(this));

            flashSwap.flashSwap_V2_to_V3({
                pool0: DAI_WETH_V2_POOL,
                fee1: FEE_1,
                tokenIn: WETH,
                tokenOut: DAI,
                amountIn: testAmounts[i]
            });

            uint256 profit = weth.balanceOf(address(this)) - balanceBefore;
            assertGt(profit, 0, "No profit generated");
            console2.log("Profit for amount %e: %e", testAmounts[i], profit);
        }
    }

    receive() external payable {}
}
