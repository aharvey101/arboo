// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
address constant uniswapV2Router = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
address constant v3_quoter = 0x61fFE014bA17989E743c5F6cB21bF9697530B21e;

contract Quoter {
    IQuoterV2 quoter = IQuoterV2(v3_quoter);
    IUniswapV2Router02 v2_quoter = IUniswapV2Router02(uniswapV2Router);

    function getUniswapV2Quote(
        address tokenIn,
        address tokenOut,
        uint256 amountIn
    ) external view returns (uint256 amountOut) {
        address[] memory path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;

        uint[] memory amounts = IUniswapV2Router02(uniswapV2Router)
            .getAmountsOut(amountIn, path);
        amountOut = amounts[1];
    }

    function getUniswapV3Quote(
        address[] memory path,
        uint256 amountIn
    ) external returns (uint256 amountOut) {
        (amountOut, , , ) = quoter.quoteExactInput(
            abi.encodePacked(path),
            amountIn
        );
        return amountOut;
    }
}

interface IUniswapV2Router02 {
    function getAmountsOut(
        uint amountIn,
        address[] calldata path
    ) external view returns (uint[] memory amounts);
}

interface IQuoterV2 {
    function quoteExactInput(
        bytes memory path,
        uint256 amountIn
    )
        external
        returns (
            uint256 amountOut,
            uint160[] memory sqrtPriceX96AfterList,
            uint32[] memory initializedTicksCrossedList,
            uint256 gasEstimate
        );
}
