// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

address constant SWAP_ROUTER_02 = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

error UnderflowError(uint256 buyBackAmount, uint256 amountIn);
error AmountLessThanZero();
error NotSender(address sender);
error BuyBackAmountLessThanAmountIn(uint256 buyBackAmount, uint256 amountIn);
error ProfitIsZero();
error InsufficientBalance();

contract V2FlashToV3Swap {
    address public owner;
    ISwapRouter02 constant router = ISwapRouter02(SWAP_ROUTER_02);
    IUniswapV2Router02 constant v2_router = IUniswapV2Router02(UNISWAP_V2_ROUTER);
    
    uint160 private constant MIN_SQRT_RATIO = 4295128739;
    uint160 private constant MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342;

    constructor() {
        owner = msg.sender;
    }

    function withdraw() external {
        if (msg.sender != owner) {
            revert("Only owner can withdraw");
        }

        // Try to get WETH balance, handle case where WETH doesn't exist
        try IWETH(WETH).balanceOf(address(this)) returns (uint256 balance) {
            if (balance > 0) {
                IWETH(WETH).withdraw(balance);
                payable(msg.sender).transfer(balance);
            }
        } catch {
            // WETH doesn't exist or call failed, just send any ETH balance
            uint256 ethBalance = address(this).balance;
            if (ethBalance > 0) {
                payable(msg.sender).transfer(ethBalance);
            }
        }
    }

    // Flash swap from V2 pool to swap on V3 pool and back
    function flashSwap_V2_to_V3(
        address v2Pool,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint24 v3Fee
    ) external {
        // Determine which token is token0 and token1 in the V2 pool
        address token0 = IUniswapV2Pool(v2Pool).token0();
        address token1 = IUniswapV2Pool(v2Pool).token1();
        
        uint256 amount0Out = 0;
        uint256 amount1Out = 0;
        
        // Set the amount to borrow based on which token we want
        if (tokenIn == token0) {
            amount0Out = amountIn;
        } else if (tokenIn == token1) {
            amount1Out = amountIn;
        } else {
            revert("Token not in pool");
        }

        // Encode callback data
        bytes memory data = abi.encode(
            msg.sender,
            v2Pool,
            tokenIn,
            tokenOut,
            amountIn,
            v3Fee
        );

        // Initiate flash swap on V2 pool
        IUniswapV2Pool(v2Pool).swap(amount0Out, amount1Out, address(this), data);
    }

    // V3 swap function
    function _swap_v3(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint256 amountOutMin
    ) private returns (uint256 amountOut) {
        IERC20(tokenIn).approve(SWAP_ROUTER_02, amountIn);

        bool zeroForOne = tokenIn < tokenOut;
        uint160 sqrtPriceLimitX96 = zeroForOne ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1;

        ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
            .ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: address(this),
                amountIn: amountIn,
                amountOutMinimum: amountOutMin,
                sqrtPriceLimitX96: sqrtPriceLimitX96
            });

        return router.exactInputSingle(params);
    }

    // Calculate the amount to repay for V2 flash swap
    function _getAmountToRepay(
        address tokenIn,
        address tokenOut,
        uint256 amountBorrowed,
        address v2Pool
    ) private view returns (uint256) {
        // V2 charges 0.3% fee
        // Amount to repay = amountBorrowed * 1000 / 997 (rounded up)
        uint256 numerator = amountBorrowed * 1000;
        uint256 denominator = 997;
        return (numerator + denominator - 1) / denominator;
    }

    // V2 flash swap callback
    function uniswapV2Call(
        address sender,
        uint256 amount0,
        uint256 amount1,
        bytes calldata data
    ) external {
        // Decode callback data
        (
            address caller,
            address v2Pool,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
            uint24 v3Fee
        ) = abi.decode(data, (address, address, address, address, uint256, uint24));

        // Verify caller is the correct V2 pool
        if (msg.sender != v2Pool) {
            revert NotSender(msg.sender);
        }

        // Get the borrowed amount (either amount0 or amount1)
        uint256 amountBorrowed = amount0 > 0 ? amount0 : amount1;
        
        if (amountBorrowed != amountIn) {
            revert("Borrowed amount mismatch");
        }

        // Swap on V3: tokenIn -> tokenOut
        uint256 amountOut = _swap_v3({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: v3Fee,
            amountIn: amountBorrowed,
            amountOutMin: 1
        });

        // Calculate how much we need to repay to V2 pool (including 0.3% fee)
        uint256 amountToRepay = _getAmountToRepay(tokenIn, tokenOut, amountBorrowed, v2Pool);

        // Swap back on V2: tokenOut -> tokenIn to get repayment amount
        uint256 buyBackAmount = _swap_v2_for_repayment(tokenOut, tokenIn, amountOut, amountToRepay);

        if (buyBackAmount < amountToRepay) {
            revert BuyBackAmountLessThanAmountIn(buyBackAmount, amountToRepay);
        }

        uint256 profit = buyBackAmount - amountToRepay;
        if (profit == 0) {
            revert ProfitIsZero();
        }

        // Repay V2 pool
        IERC20(tokenIn).transfer(v2Pool, amountToRepay);

        // Send profit to caller
        if (tokenIn != WETH) {
            IERC20(tokenIn).transfer(caller, profit);
        } else {
            IWETH(WETH).transfer(caller, profit);
        }
    }

    // Swap on V2 to get enough tokens to repay the flash loan
    function _swap_v2_for_repayment(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOutMin
    ) private returns (uint256 amountOut) {
        IERC20(tokenIn).approve(address(v2_router), amountIn);

        address[] memory path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;

        uint256[] memory amounts = v2_router.swapExactTokensForTokens(
            amountIn,
            amountOutMin,
            path,
            address(this),
            block.timestamp
        );

        if (amounts[1] <= 0) {
            revert AmountLessThanZero();
        }
        
        return amounts[1];
    }
}

// Interfaces
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

    function swapTokensForExactTokens(
        uint256 amountOut,
        uint256 amountInMax,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
    
    function getAmountsOut(
        uint amountIn,
        address[] calldata path
    ) external view returns (uint[] memory amounts);
}

interface IUniswapV2Pool {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
}

interface IERC20 {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
}

interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint256 amount) external;
}
