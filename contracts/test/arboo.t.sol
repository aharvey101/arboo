// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console2} from "forge-std/Test.sol";
import {UniswapV3FlashSwap, IERC20} from "../src/arboo.sol";

contract UniswapV3FlashTest is Test {
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant MKR = 0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2;
    // DAI / MKR 0.3% fee
    address constant POOL = 0x60594a405d53811d3BC4766596EFD80fd545A270;
    uint24 constant POOL_FEE = 3000;

    IERC20 private constant mkr = IERC20(MKR);
    IERC20 private constant dai = IERC20(DAI);
    UniswapV3FlashSwap private uni;
    address constant user = address(11);

    function setUp() public {
        uni = new UniswapV3FlashSwap(POOL);

        deal(DAI, user, 1e6 * 1e18);
        vm.prank(user);
        dai.approve(address(uni), type(uint256).max);
    }

    function test_flash_swap() public {
        uint256 dai_before = dai.balanceOf(user);
        vm.prank(user);
        uni.flash(1e6 * 1e18, 0);
        uint256 dai_after = dai.balanceOf(user);

        uint256 fee = dai_before - dai_after;
        console2.log("DAI fee", fee);
    }
}
