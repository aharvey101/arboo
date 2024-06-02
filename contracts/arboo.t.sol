
pragma solidity 0.8.20;
import "@forge-std/Test.sol";
import "@forge-std/console.sol";
import '@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol'
import "./arboo.sol";
import '@uniswap/v3-core/contracts/libraries/LowGasSafeMath.sol';
import '@uniswap/v3-periphery/contracts/base/PeripheryPayments.sol';
import '@uniswap/v3-periphery/contracts/base/PeripheryImmutableState.sol';
import '@uniswap/v3-periphery/contracts/libraries/PoolAddress.sol';
import '@uniswap/v3-periphery/contracts/libraries/CallbackValidation.sol';
import '@uniswap/v3-periphery/contracts/libraries/TransferHelper.sol';
import '@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol';


function initFlash(FlashParams memory params);

contract ArbooTest is Test {

    // test that initFlash and uniswapV3FlashCallback work as intended

    Arbooo bot

    IWETH weth = IWETH(0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2);

    function test() public {
        console.log("test starting")
    }

}
