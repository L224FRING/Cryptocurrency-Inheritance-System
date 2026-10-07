```bash
export PATH="$PATH:$HOME/.foundry/bin:$HOME/.cargo/bin"
```

```bash
./rust/frost-service/target/debug/frost-service selftest
```

```bash
./rust/frost-service/target/debug/frost-service matrix
```

```bash
./rust/frost-service/target/debug/frost-service vdf selftest
```

```bash
./rust/frost-service/target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5
```

```bash
export RELAY=http://127.0.0.1:8477
export SESSION=$(python3 -c "import os;print(os.urandom(32).hex())")
mkdir -p /tmp/shares

for i in 1 2 3 4 5; do
  ./rust/frost-service/target/debug/frost-service dkg-party \
    --index $i --trustees 5 --threshold 3 \
    --session $SESSION --relay $RELAY \
    --out /tmp/shares/share-$i.json &
done
wait

for i in 1 2 3 4 5; do
  python3 -c "import json;print(json.load(open('/tmp/shares/share-$i.json'))['group_verifying_key'])"
done
```

```bash
export GROUP_PUBKEY=$(python3 -c "import json;print(json.load(open('/tmp/shares/share-1.json'))['group_verifying_key'])")
```

```bash
anvil
```

```bash
export OWNER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
export BENEFICIARY=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
export ANVIL_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
export GROUP_PUBKEY=$(python3 -c "import json;print(json.load(open('/tmp/shares/share-1.json'))['group_verifying_key'])")

forge script script/DeployFull.s.sol \
  --rpc-url http://127.0.0.1:8545 \
  --broadcast \
  --private-key $ANVIL_KEY
```

```bash
export FROST_ADDR=<FROSTVerifier address>
export VDF_ADDR=<VDFVerifier address>
export VAULT_ADDR=<InheritanceVault address>
```

```bash
cast send $FROST_ADDR "setGroupPublicKeyCompressed(bytes)" $GROUP_PUBKEY \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER
```

```bash
cast send $VAULT_ADDR "setBeneficiary(address)" $BENEFICIARY \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER

cast send $VAULT_ADDR --value 10ether \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER

cast call $VAULT_ADDR "beneficiarySet()(bool)" --rpc-url http://127.0.0.1:8545
cast balance $VAULT_ADDR --rpc-url http://127.0.0.1:8545
```

```bash
cast send $VAULT_ADDR "checkIn()" \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER
```

```bash
CHALLENGE=$(cast call $VDF_ADDR "getCurrentChallenge()(uint256)" --rpc-url http://127.0.0.1:8545 | awk '{print $1}')
echo "Challenge: $CHALLENGE"
T_VAL=$(cast call $VDF_ADDR "T()(uint256)" --rpc-url http://127.0.0.1:8545)
DELAY=$(cast call $VDF_ADDR "requiredDelay()(uint256)" --rpc-url http://127.0.0.1:8545)
echo "T: $T_VAL  requiredDelay: $DELAY"
```

```bash
CHEX=$(python3 -c "print(hex($CHALLENGE)[2:])")
./rust/frost-service/target/debug/frost-service vdf --t $T_VAL --input 0x$CHEX > /tmp/vdf.json
cat /tmp/vdf.json
```

```bash
Y=$(python3 -c "import json;print(int(json.load(open('/tmp/vdf.json'))['y'],16))")
PROOF=$(python3 -c "import json;d=json.load(open('/tmp/vdf.json'));print('['+','.join(str(int(p,16)) for p in d['proof_points'])+']')")
echo "y=$Y"; echo "proof=$PROOF"
```

```bash
cast rpc anvil_mine 0x10 --rpc-url http://127.0.0.1:8545
```

```bash
cast send $VDF_ADDR "submitVDFProof(uint256,uint256[])" $Y "$PROOF" \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $BENEFICIARY
```

```bash
cast call $VDF_ADDR "isInactivityConfirmed()(bool)" --rpc-url http://127.0.0.1:8545
```

```bash
cast send $VAULT_ADDR "checkIn()" --unlocked --from $OWNER
```

```bash
./rust/frost-service/target/debug/frost-service attest --message "death-confirmed"
```

```bash
export RELAY=http://127.0.0.1:8477
export SIGSESSION=$(python3 -c "import os;print(os.urandom(32).hex())")

./rust/frost-service/target/debug/frost-service sign-party \
  --index 1 --share /tmp/shares/share-1.json \
  --participants 1,2,3 --session $SIGSESSION \
  --relay $RELAY --aggregate > /tmp/sign1.json &

./rust/frost-service/target/debug/frost-service sign-party \
  --index 2 --share /tmp/shares/share-2.json \
  --participants 1,2,3 --session $SIGSESSION --relay $RELAY &

./rust/frost-service/target/debug/frost-service sign-party \
  --index 3 --share /tmp/shares/share-3.json \
  --participants 1,2,3 --session $SIGSESSION --relay $RELAY &
wait

SIG=$(python3 -c "import json;print(json.load(open('/tmp/sign1.json'))['signature'])")
echo "signature=$SIG"
```

```bash
cast send $VAULT_ADDR "release(bytes)" 0x$SIG \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $BENEFICIARY
```

```bash
cast call $VAULT_ADDR "isReleased()(bool)" --rpc-url http://127.0.0.1:8545
cast balance $VAULT_ADDR --rpc-url http://127.0.0.1:8545
cast balance $BENEFICIARY --rpc-url http://127.0.0.1:8545
```

```bash
pkill -f frost-relay
```
