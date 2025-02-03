// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;
import {Test, console2} from "forge-std/Test.sol";

address constant SWAP_ROUTER_02 = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

contract UniswapV2FlashSwap {
    ISwapRouter02 constant router = ISwapRouter02(SWAP_ROUTER_02);
    IUniswapV2Router02 constant v2_router =
        IUniswapV2Router02(UNISWAP_V2_ROUTER);
    uint160 private constant MIN_SQRT_RATIO = 4295128739;
    uint160 private constant MAX_SQRT_RATIO =
        1461446703485210103287273052203988822378723970342;

    // Error declarations
    error NotProfitable(uint256 buyBackAmount, uint256 amountToRepay);
    error NotSender();
    error InsufficientOutputAmount();

    function flashSwap_V2_to_V3(
        address pool0, // V2 pool to borrow from
        uint24 fee1, // V3 pool fee
        address tokenIn,
        address tokenOut,
        uint256 amountIn
    ) external {
        // Get amount0 and amount1 based on token ordering
        (uint256 amount0Out, uint256 amount1Out) = tokenIn < tokenOut
            ? (uint256(0), amountIn)
            : (amountIn, uint256(0));
        console2.log("Amount 0 Out:", amount0Out);
        console2.log("Amount 1 Out:", amount1Out);
        // Encode callback data
        bytes memory data = abi.encode(
            msg.sender, // original caller
            pool0, // V2 pool address
            fee1, // V3 pool fee
            tokenIn, // token we're borrowing
            tokenOut, // token we're swapping to
            amountIn // amount we're borrowing
        );
        console2.log("Test");
        IUniswapV2Pool(pool0).swap(amount0Out, amount1Out, address(this), data);
    }

    function _v3_swap(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn
    ) private returns (uint256 amountOut) {
        IERC20(tokenIn).approve(SWAP_ROUTER_02, amountIn);

        ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
            .ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: address(this),
                amountIn: amountIn,
                amountOutMinimum: 0, // Note: In production, you should set a minimum
                sqrtPriceLimitX96: 0
            });

        amountOut = router.exactInputSingle(params);

        if (amountOut == 0) revert InsufficientOutputAmount();
        return amountOut;
    }

    // This is called by the V2 pool after we receive the flash loaned tokens
    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external {
        console2.log("Amount to repay?", amount0);

        (
            address caller,
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn
        ) = abi.decode(
                data,
                (address, address, uint24, address, address, uint256)
            );

        // Verify caller
        if (msg.sender != pool0) revert NotSender();
        if (sender != address(this)) revert NotSender();

        // Calculate the amount we need to repay
        uint256 amountToRepay = amountIn + 1; // 0.3% fee + 1 wei for rounding
        console2.log("Amount to repay?", amountToRepay);
        // Get the amount we received from the flash loan

        uint256 amountReceived = amount0 > 0 ? amount0 : amount1;
        console2.log("Amount recieved", amountReceived);
        // Do the V3 swap with the borrowed tokens
        uint256 buyBackAmount = _v3_swap(
            tokenOut, // token we received from V2
            tokenIn, // token we need to repay
            fee1, // V3 pool fee
            amountReceived
        );
        console2.log("Buy Back Amount", buyBackAmount);
        // Check if profitable
        if (buyBackAmount <= amountToRepay) {
            revert NotProfitable(buyBackAmount, amountToRepay);
        }

        // Calculate profit
        uint256 profit = buyBackAmount - amountToRepay;

        // Repay the V2 pool
        IERC20(tokenIn).transfer(msg.sender, amountToRepay);

        // Send profit to original caller
        if (tokenIn == WETH) {
            // If profit is in WETH, send directly
            IERC20(WETH).transfer(caller, profit);
        } else {
            // If profit is in another token, swap to WETH first
            IERC20(tokenIn).approve(SWAP_ROUTER_02, profit);
            ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
                .ExactInputSingleParams({
                    tokenIn: tokenIn,
                    tokenOut: WETH,
                    fee: 500, // Use lowest fee pool for profit conversion
                    recipient: caller,
                    amountIn: profit,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                });
            router.exactInputSingle(params);
        }
    }
}

interface ISwapRouter02 {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    function exactInputSingle(
        ExactInputSingleParams calldata params
    ) external payable returns (uint256 amountOut);
}

interface IUniswapV2Router02 {
    function swapExactTokensForTokens(
        uint amountIn,
        uint amountOutMin,
        address[] calldata path,
        address to,
        uint deadline
    ) external returns (uint[] memory amounts);
}

interface IUniswapV2Pool {
    function swapExactTokensForTokens(
        uint amountIn,
        uint amountOutMin,
        address[] calldata path,
        address to,
        uint deadline
    ) external returns (uint[] memory amounts);

    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
}

interface IERC20 {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(
        address recipient,
        uint256 amount
    ) external returns (bool);
    function allowance(
        address owner,
        address spender
    ) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(
        address sender,
        address recipient,
        uint256 amount
    ) external returns (bool);
}
interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint256 amount) external;
}

interface IUniswapV3Pool {
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1);
}
