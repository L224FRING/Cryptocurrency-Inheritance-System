// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {console2} from "forge-std/console2.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";

contract EvaluationTest is Test {
    string internal constant FROST_BIN = "rust/frost-service/target/debug/frost-service";

    uint256 internal constant VDF_N = 604472132917888474175096;
    string internal constant VDF_N_HEX = "0x80007fff123368ac5678";
    uint256 internal constant VDF_T = 100;
    uint256 internal constant VDF_X = 4660;
    uint256 internal constant VDF_Y = 244925934411125229258464;

    bytes internal constant KAT_GROUP_KEY = hex"037c2de6c714cc97e1575c17108305e37b6508e55fc16d724b9cceeaa16314a3d8";
    bytes internal constant KAT_SIGNATURE = hex"0352cf9dc6b1fd9ea023fcbd7e1ad627e901c2217d295420fe2773d08eb083fe45"
        hex"fb67cd0a5917e7bc98b047652a5cd84a6462f74923c27be708471b62f5b3696f";
    bytes32 internal constant KAT_MESSAGE = 0xa1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90;

    FROSTVerifier internal frost;
    VDFVerifier internal vdf;
    InheritanceVault internal vault;
    address internal owner;
    address internal beneficiary;

    function setUp() public {
        owner = makeAddr("owner");
        beneficiary = makeAddr("beneficiary");
        vm.startPrank(owner);
        vdf = new VDFVerifier(VDF_N, VDF_T, 5);
        frost = new FROSTVerifier();
        vault = new InheritanceVault(address(vdf), address(frost));
        vdf.bindVault(address(vault));
        vault.setBeneficiary(beneficiary);
        vm.stopPrank();
    }

    function _installKey(bytes memory groupKey) internal {
        vm.prank(owner);
        frost.setGroupPublicKeyCompressed(groupKey);
    }

    function _katProof() internal pure returns (uint256[] memory proof) {
        proof = new uint256[](7);
        proof[0] = 202387890901749777132256;
        proof[1] = 185435587741127332234456;
        proof[2] = 524677350528052040861944;
        proof[3] = 585707853613048732848016;
        proof[4] = 444558425012851277948448;
        proof[5] = 381063172646044478796808;
        proof[6] = 83625018937571556362864;
    }

    function _proveVDF(uint256 t) internal returns (uint256 y, uint256[] memory proof) {
        string[] memory cmd = new string[](8);
        cmd[0] = FROST_BIN;
        cmd[1] = "vdf";
        cmd[2] = "--t";
        cmd[3] = vm.toString(t);
        cmd[4] = "--n";
        cmd[5] = VDF_N_HEX;
        cmd[6] = "--input";
        cmd[7] = "0x1234";

        string memory json = string(vm.ffi(cmd));
        y = vm.parseUint(string.concat("0x", vm.parseJsonString(json, ".y")));

        string[] memory points = vm.parseJsonStringArray(json, ".proof_points");
        proof = new uint256[](points.length);
        for (uint256 i = 0; i < points.length; i++) {
            proof[i] = vm.parseUint(points[i]);
        }
    }

    function _signReleaseHash(bytes32 releaseHash) internal returns (bytes memory signature, bytes memory groupKey) {
        string[] memory cmd = new string[](11);
        cmd[0] = FROST_BIN;
        cmd[1] = "sign";
        cmd[2] = "--trustees";
        cmd[3] = "5";
        cmd[4] = "--threshold";
        cmd[5] = "3";
        cmd[6] = "--participants";
        cmd[7] = "1,2,3";
        cmd[8] = "--raw-message";
        cmd[9] = "--message";
        cmd[10] = vm.toString(releaseHash);

        string memory json = string(vm.ffi(cmd));
        signature = vm.parseBytes(string.concat("0x", vm.parseJsonString(json, ".signing.signature")));
        groupKey = vm.parseBytes(string.concat("0x", vm.parseJsonString(json, ".dkg.group_verifying_key")));
    }

    function _confirmVDF() internal {
        // The proof is KAT-bound to x = 0x1234, so point the live challenge at
        // that value and wait out requiredDelay before submitting.
        vm.store(address(vdf), bytes32(uint256(2)), bytes32(uint256(VDF_X)));
        vm.roll(block.number + 10);
        vdf.submitVDFProof(VDF_Y, _katProof());
    }

    function _releaseHash() internal view returns (bytes32) {
        return keccak256(
            abi.encodePacked(
                "inheritance-release", block.chainid, address(vault), beneficiary, vault.getCurrentChallenge()
            )
        );
    }

    function test_Gas_VaultCheckIn() public {
        vm.prank(owner);
        uint256 g = gasleft();
        vault.checkIn();
        g -= gasleft();
        console2.log("GAS vault.checkIn", g);
    }

    function test_Gas_VDFCheckIn() public {
        vm.prank(owner);
        uint256 g = gasleft();
        vdf.checkIn();
        g -= gasleft();
        console2.log("GAS vdf.checkIn", g);
    }

    function test_Gas_SubmitVDFProof() public {
        vm.store(address(vdf), bytes32(uint256(2)), bytes32(uint256(VDF_X)));
        vm.roll(block.number + 10);

        uint256[] memory proof = _katProof();
        uint256 g = gasleft();
        vdf.submitVDFProof(VDF_Y, proof);
        g -= gasleft();
        console2.log("GAS vdf.submitVDFProof", g);
        assertTrue(vdf.isInactivityConfirmed());
    }

    function test_Gas_FrostVerify() public {
        _installKey(KAT_GROUP_KEY);
        uint256 g = gasleft();
        frost.verifyFROSTSignature(KAT_MESSAGE, KAT_SIGNATURE);
        g -= gasleft();
        console2.log("GAS frost.verifyFROSTSignature", g);
    }

    function test_Gas_Release() public {
        bytes32 releaseHash = _releaseHash();
        (bytes memory signature, bytes memory groupKey) = _signReleaseHash(releaseHash);
        _installKey(groupKey);

        _confirmVDF();
        vm.deal(address(vault), 1 ether);

        uint256 g = gasleft();
        vault.release(signature);
        g -= gasleft();
        console2.log("GAS vault.release", g);

        assertTrue(vault.isReleased());
        assertEq(beneficiary.balance, 1 ether);
    }

    function test_Security_EarlyProofRejected() public {
        vm.expectRevert("too early");
        vdf.submitVDFProof(VDF_Y, _katProof());
    }

    function test_DualGate_RequiresVDF() public {
        vm.expectRevert("vdf not confirmed");
        vault.release(KAT_SIGNATURE);
    }

    function test_DualGate_RequiresSignature() public {
        _installKey(KAT_GROUP_KEY);
        _confirmVDF();
        vm.expectRevert("invalid signature");
        vault.release(KAT_SIGNATURE);
    }

    function test_DualGate_ReleasesWithBoth() public {
        bytes32 releaseHash = _releaseHash();
        (bytes memory signature, bytes memory groupKey) = _signReleaseHash(releaseHash);
        _installKey(groupKey);

        _confirmVDF();

        vault.release(signature);
        assertTrue(vault.isReleased());
    }

    function test_Security_DoubleReleaseRejected() public {
        bytes32 releaseHash = _releaseHash();
        (bytes memory signature, bytes memory groupKey) = _signReleaseHash(releaseHash);
        _installKey(groupKey);

        _confirmVDF();
        vault.release(signature);

        vm.expectRevert("already released");
        vault.release(signature);
    }

    function test_VDF_GasScaling() public {
        uint256[8] memory ts = [uint256(8), 16, 32, 64, 128, 256, 512, 1024];
        for (uint256 i = 0; i < ts.length; i++) {
            (uint256 y, uint256[] memory proof) = _proveVDF(ts[i]);
            uint256 g = gasleft();
            bool ok = vdf.verifyVDF(VDF_X, y, proof, ts[i], VDF_N);
            g -= gasleft();
            assertTrue(ok, "scaling proof must verify");
            console2.log(
                string.concat("GAS verifyVDF T=", vm.toString(ts[i]), " points=", vm.toString(proof.length)), g
            );
        }
    }
}
