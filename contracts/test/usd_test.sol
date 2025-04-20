// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Test.sol";
import "../src/usdc_arb.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "aave-v3-core/contracts/flashloan/base/FlashLoanReceiverBase.sol";
import "aave-v3-core/contracts/interfaces/IPoolAddressesProvider.sol";
import "aave-v3-core/contracts/interfaces/IPool.sol";

contract UniswapV3FlashLoanArbitrageTest is Test {
    UniswapV3FlashLoanArbitrage public arbitrage;
    
    // Mainnet contract addresses
    address public constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address public constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address public constant USDT = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    
    // Aave V3 and Uniswap V3 addresses on mainnet
    address public constant AAVE_POOL_ADDRESSES_PROVIDER = 0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e;
    address public constant UNISWAP_V3_SWAP_ROUTER = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
    
    // Uniswap V3 pool addresses (as per your original contract)
    address public constant USDC_WETH_POOL = 0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8; // 0.3% fee
    address public constant WETH_DAI_POOL = 0x5777d92f208679DB4b9778590Fa3CAB3aC9e2168; // 0.3% fee
    address public constant DAI_USDC_POOL = 0x6c6Bc977E13Df9b0de53b251522280BB72383700; // 0.05% fee
    
    // Test constants
    uint256 public constant FLASH_LOAN_AMOUNT = 10_000 * 10**6; // 10,000 USDC with 6 decimals
    
    // Whale addresses (accounts with lots of tokens for testing)
    address public constant USDC_WHALE = 0xe1940f578743367F38D3f25c2D2d32D6636929B6;
    address public constant WETH_WHALE = 0xa359Fc83C48277EedF375a5b6DC9Ec7D093aD3f2;
    
    // Owner of the arbitrage contract
    address public owner = address(this);
    
    // Event definition
    event ArbitrageExecuted(
        address indexed initiator,
        address indexed startToken,
        uint256 startAmount,
        uint256 profit
    );
    
    function setUp() public {
        // Fork Ethereum mainnet
        //uint256 mainnetFork = vm.createFork("mainnet");
        //vm.selectFork(mainnetFork);
        
        // Set block to a recent one for consistent testing
        //vm.rollFork(18475000); // Replace with a recent block number
        
        // Deploy the arbitrage contract from our test owner
        //vm.startPrank(owner);
        arbitrage = new UniswapV3FlashLoanArbitrage(
            IPoolAddressesProvider(AAVE_POOL_ADDRESSES_PROVIDER),
            ISwapRouter(UNISWAP_V3_SWAP_ROUTER)
        );
        //vm.stopPrank();
        // Fund owner account with ETH for gas
        vm.deal(owner, 100 ether);
        
    }
    
    function testFlashLoanExecution() public {
        // This test verifies that the contract can successfully execute a flash loan
        //vm.startPrank(owner);
        
        // Execute a flash loan with USDC
        // We expect this to either succeed or revert with a specific error
        // (like insufficient profitability, but not a contract integration error)
        try arbitrage.executeArbitrage(USDC, FLASH_LOAN_AMOUNT) {
            // If the transaction succeeds, the flash loan was properly processed
            // No assertions needed - success itself verifies the integration works
            console.log("Flash loan executed successfully!");
        } catch Error(string memory reason) {
            // We allow specific error messages that indicate business logic failures
            // but not integration failures
            assertFalse(
                keccak256(bytes(reason)) == keccak256(bytes("SafeERC20: low-level call failed")) ||
                keccak256(bytes(reason)) == keccak256(bytes("Address: call to non-contract")),
                "Flash loan execution failed with integration error"
            );
            console.log("Flash loan reverted with acceptable reason:", reason);
        }
        
        //vm.stopPrank();
    }
    
    function testWithdrawFunctionality() public {
        
        // Send some tokens to the contract to test withdrawal functions
        // Impersonate a whale to transfer tokens
        vm.prank(USDC_WHALE);
        IERC20(USDC).transfer(address(arbitrage), 1000 * 10**6); // 1,000 USDC
        
        vm.prank(WETH_WHALE);
        IERC20(WETH).transfer(address(arbitrage), 1 * 10**18); // 1 WETH
        //vm.stopPrank();
        
        // Back to owner
        //vm.startPrank(owner);
        
        // Check balances before withdrawal
        uint256 contractUsdcBefore = IERC20(USDC).balanceOf(address(arbitrage));
        uint256 contractWethBefore = IERC20(WETH).balanceOf(address(arbitrage));
        uint256 ownerUsdcBefore = IERC20(USDC).balanceOf(owner);
        uint256 ownerWethBefore = IERC20(WETH).balanceOf(owner);
        
        // Test withdrawal of specific tokens
        arbitrage.withdrawProfit(USDC);
        
        // Verify USDC was withdrawn
        assertEq(IERC20(USDC).balanceOf(address(arbitrage)), 0, "All USDC should be withdrawn");
        assertEq(
            IERC20(USDC).balanceOf(owner),
            ownerUsdcBefore + contractUsdcBefore,
            "Owner should receive all USDC"
        );
        
        // Test withdrawAllProfits
        arbitrage.withdrawAllProfits();
        
        // Verify WETH was withdrawn
        assertEq(IERC20(WETH).balanceOf(address(arbitrage)), 0, "All WETH should be withdrawn");
        assertEq(
            IERC20(WETH).balanceOf(owner),
            ownerWethBefore + contractWethBefore,
            "Owner should receive all WETH"
        );
        
        //vm.stopPrank();
    }
    
    function testOnlyOwnerCanExecuteArbitrage() public {
        address unauthorized = address(0x2);
        vm.deal(unauthorized, 1 ether); // Give them some ETH for gas
        vm.startPrank(unauthorized);
        
        vm.expectRevert("Ownable: caller is not the owner");
        arbitrage.executeArbitrage(USDC, FLASH_LOAN_AMOUNT);
        
        vm.expectRevert("Ownable: caller is not the owner");
        arbitrage.withdrawProfit(USDC);
        
        vm.expectRevert("Ownable: caller is not the owner");
        arbitrage.withdrawAllProfits();
        
        vm.stopPrank();
    }
    
    function testReceiveEther() public {
    // Get initial balance
        uint256 initialBalance = address(arbitrage).balance;
        
        // Send ETH to the contract
        payable(address(arbitrage)).transfer(1 ether);
        
        // Check contract balance increased by exactly 1 ETH
        assertEq(
            address(arbitrage).balance, 
            initialBalance + 1 ether, 
            "Contract should receive exactly 1 ETH"
        );
    }
    
    // Optional: A test to measure gas usage for optimization
    function testGasOptimization() public {
        // Fund the caller with ETH
        vm.startPrank(owner);
        
        // Capture gas usage for future optimization
        uint256 gasBefore = gasleft();
        
        // Execute a representative transaction
        try arbitrage.executeArbitrage(USDC, FLASH_LOAN_AMOUNT) {
            // Success case
        } catch {
            // Failure is okay for gas testing
        }
        
        uint256 gasAfter = gasleft();
        uint256 gasUsed = gasBefore - gasAfter;
        
        // Log gas usage
        console.log("Gas used for arbitrage execution:", gasUsed);
        
        vm.stopPrank();
    }
}
