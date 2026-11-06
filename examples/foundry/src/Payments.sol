// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract Payments {
    event PaymentReceived(address indexed payer, uint256 amount, bytes32 referenceId);
    function pay(bytes32 referenceId) external payable {
        emit PaymentReceived(msg.sender, msg.value, referenceId);
    }
}
