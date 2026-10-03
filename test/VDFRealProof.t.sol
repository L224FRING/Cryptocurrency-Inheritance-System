// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";

/// @notice Known-answer test: the proof below was produced by the Rust service
/// (`frost-service vdf --t 100 --input 0x1234`) after the Fiat-Shamir challenge
/// derivation was aligned with the Solidity verifier (keccak256 over 32-byte
/// big-endian words). If either side drifts, this test fails.
///
/// Regenerate with:
///   cargo build --manifest-path rust/frost-service/Cargo.toml
///   ./rust/frost-service/target/debug/frost-service vdf --t 100 --input 0x1234
contract VDFRealProofTest is Test {
    uint256 constant N = 604472132917888474175096;
    uint256 constant T = 100;

    VDFVerifier internal vdf;

    function setUp() public {
        vdf = new VDFVerifier(N, T, 0);
    }

    function test_RustProofVerifiesOnChain() public view {
        uint256 x = 4660; // 0x1234
        uint256 y = 244925934411125229258464;

        uint256[] memory proof = new uint256[](7);
        proof[0] = 202387890901749777132256;
        proof[1] = 185435587741127332234456;
        proof[2] = 524677350528052040861944;
        proof[3] = 585707853613048732848016;
        proof[4] = 444558425012851277948448;
        proof[5] = 381063172646044478796808;
        proof[6] = 83625018937571556362864;

        assertTrue(vdf.verifyVDF(x, y, proof, T, N), "rust proof must verify");
    }

    function test_TamperedProofRejected() public view {
        uint256 x = 4660;
        uint256 y = 244925934411125229258464;

        uint256[] memory proof = new uint256[](7);
        proof[0] = 202387890901749777132256;
        proof[1] = 185435587741127332234456;
        proof[2] = 524677350528052040861944;
        proof[3] = 585707853613048732848016;
        proof[4] = 444558425012851277948448;
        proof[5] = 381063172646044478796808;
        proof[6] = 83625018937571556362864;
        proof[2] = proof[2] + 1; // tamper

        assertFalse(vdf.verifyVDF(x, y, proof, T, N), "tampered proof must fail");
    }

    function test_WrongOutputRejected() public view {
        uint256 x = 4660;
        uint256 y = 244925934411125229258464 + 1;

        uint256[] memory proof = new uint256[](7);
        proof[0] = 202387890901749777132256;
        proof[1] = 185435587741127332234456;
        proof[2] = 524677350528052040861944;
        proof[3] = 585707853613048732848016;
        proof[4] = 444558425012851277948448;
        proof[5] = 381063172646044478796808;
        proof[6] = 83625018937571556362864;

        assertFalse(vdf.verifyVDF(x, y, proof, T, N), "wrong output must fail");
    }
}
