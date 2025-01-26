// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;
import {Test, console2} from "forge-std/Test.sol";

address constant SWAP_ROUTER_02 = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
address constant DAIETHPOOL = 0xA478c2975Ab1Ea89e8196811F51A7B7Ade33eB11;

contract UniswapV3FlashSwap {
    ISwapRouter02 constant router = ISwapRouter02(SWAP_ROUTER_02);
    IUniswapV2Router02 constant v2_router = IUniswapV2Router02(UNISWAP_V2_ROUTER);
    uint160 private constant MIN_SQRT_RATIO = 4295128739;
    uint160 private constant MAX_SQRT_RATIO =
        1461446703485210103287273052203988822378723970342;

    // DAI / WETH 0.3% swap fee (2000 DAI / WETH)
    // DAI / WETH 0.05% swap fee (2100 DAI / WETH)
    // 1. Flash swap on pool0 (receive WETH)
    // 2. Swap on pool1 (WETH -> DAI)
    // 3. Send DAI to pool0
    // profit = DAI received from pool1 - DAI repaid to pool0

    function flashSwap_V2_to_V3(
        address pool0,
        uint24 fee1,
        address tokenIn,
        address tokenOut,
        uint256 amountIn
    ) external {
        // we need to

        bool zeroForOne = tokenIn < tokenOut;
        // 0 -> 1 => sqrt price decrease
        // 1 -> 0 => sqrt price increase
        uint160 sqrtPriceLimitX96 =
            zeroForOne ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1;

        bytes memory data = abi.encode(
            msg.sender, pool0, fee1, tokenIn, tokenOut, amountIn, zeroForOne
        );

        address[] memory path;
        path = new address[](2);
        path[0] = tokenOut;
        path[1] = tokenIn;

        uint256[] memory amountOut = v2_router.getAmountsOut(amountIn, path);

        return IUniswapV2Pool(pool0).swap(amountIn, amountOut[0], address(this), data);
    }
    function flashSwap_V3_to_V2(
        address pool0,
        uint24 fee1,
        address tokenIn,
        address tokenOut,
        uint256 amountIn
    ) external {
        bool zeroForOne = tokenIn < tokenOut;
        // 0 -> 1 => sqrt price decrease
        // 1 -> 0 => sqrt price increase
        uint160 sqrtPriceLimitX96 =
            zeroForOne ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1;

        bytes memory data = abi.encode(
            msg.sender, pool0, fee1, tokenIn, tokenOut, amountIn, zeroForOne
        );

        IUniswapV3Pool(pool0).swap({
            recipient: address(this),
            zeroForOne: zeroForOne,
            amountSpecified: int256(amountIn),
            sqrtPriceLimitX96: sqrtPriceLimitX96,
            data: data
        });
    }

    function _swap_v2(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOutMin
    ) private returns (uint256 amountOut)  {
        IERC20(tokenIn).approve(address(v2_router), amountIn);

        address[] memory path;
        path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;

        uint256[] memory amounts = v2_router.swapExactTokensForTokens(
            amountIn, amountOutMin, path, address(this), block.timestamp
        );
    return amounts[1];
    }

    function v3SwapToEth( address tokenIn, address caller, uint256 profit) internal {

            IERC20(tokenIn).approve(SWAP_ROUTER_02, profit); 
            ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
                .ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: WETH,
                fee: 500,
                recipient: caller,
                amountIn: profit,
                amountOutMinimum: 0,
                sqrtPriceLimitX96: MIN_SQRT_RATIO + 1
            });
        router.exactInputSingle(params);    

    }
    function uniswapV3SwapCallback(
        int256 amount0,
        int256 amount1,
        bytes calldata data
    ) external {
        // Decode data
        (
            address caller,
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
            bool zeroForOne
        ) = abi.decode(
            data, (address, address, uint24, address, address, uint256, bool)
        );
        require(msg.sender == address(pool0), "not sender");

        uint256 amountOut = zeroForOne ? uint256(-amount1) : uint256(-amount0);

        // pool0 -> tokenIn -> tokenOut (amountOut)
        // Swap on pool 1 (swap tokenOut -> tokenIn)
        uint256 buyBackAmount = _swap_v2({
            tokenIn: tokenOut,
            tokenOut: tokenIn,
            amountIn: amountOut,
            amountOutMin: amountIn
        });

        uint256 profit = buyBackAmount - amountIn;
        require(profit > 0, "profit = 0");

        // Repay pool0
        IERC20(tokenIn).transfer(pool0, amountIn);

        if (tokenIn != WETH) {
            v3SwapToEth(tokenIn, caller, profit);
        } else {
            IWETH(WETH).transfer(address(this), profit);
        }
    }

    function _swap(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint256 amountOutMin
    ) private returns (uint256 amountOut) {
        IERC20(tokenIn).approve(address(router), amountIn);

        ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02
            .ExactInputSingleParams({
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            fee: fee,
            recipient: address(this),
            amountIn: amountIn,
            amountOutMinimum: amountOutMin,
            sqrtPriceLimitX96: 0
        });
        amountOut = router.exactInputSingle(params);
    }

    function uniswapV2Call(
           address sender,
           uint256 amount0,
           uint256 amount1,
           bytes calldata data
       ) external {
           /*require(msg.sender == address(pair), "not pair");*/
           require(sender == address(this), "not sender");

           (address tokenIn, address tokenOut, uint24 fee1, uint256 amountIn, uint256 amountOutMin) = abi.decode(data, (address,address, uint24, uint256, uint256));

           uint256 buyBackAmount = _swap({
               tokenIn: tokenOut,
               tokenOut: tokenIn,
               fee: fee1,
               amountIn: amountOutMin,
               amountOutMin: amountIn
           });

           IERC20 loanedFrom = IERC20(tokenIn);
           // about 0.3% fee, +1 to round up
           uint256 fee = (amount1 * 3) / 997 + 1;
           uint256 amountToRepay = amount1 + fee;

           // Transfer flash swap fee from caller
           loanedFrom.transferFrom(tokenOut, address(this), amountToRepay);

           // Repay
           loanedFrom.transfer(address(sender), buyBackAmount);
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

    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        returns (uint256 amountOut);
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
    function getAmountsOut(uint amountIn, address[] calldata path) external view returns (uint[] memory amounts);
    function swapExactTokensForETH(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts);
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

interface IUniswapV2Pool {
    function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
}

interface IERC20 {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount)
        external
        returns (bool);
    function allowance(address owner, address spender)
        external
        view
        returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount)
        external
        returns (bool);
}

interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint256 amount) external;
}
