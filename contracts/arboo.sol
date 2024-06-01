// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Sandooo {
    address public owner;

    bytes4 internal constant TOKEN_TRANSFER_ID = 0xa9059cbb;
    bytes4 internal constant V2_SWAP_ID = 0x022c0d9f;

    constructor() {
        owner = msg.sender;
    }

    function recoverToken(address token, uint256 amount) public {
        require(msg.sender == owner, "NOT_OWNER");

        assembly {
            switch eq(token, 0)
            case 0 {
                let ptr := mload(0x40)
                mstore(ptr, TOKEN_TRANSFER_ID)
                mstore(add(ptr, 4), caller())
                mstore(add(ptr, 36), amount)
                if iszero(call(gas(), token, 0, ptr, 68, 0, 0)) {
                    revert(0, 0)
                }
            }
            case 1 {
                if iszero(call(gas(), caller(), amount, 0, 0, 0, 0)) {
                    revert(0, 0)
                }
            }
        }
    }

    receive() external payable {}

}
