#!/bin/bash

export external="frame_system pallet_balances pallet_collator_selection pallet_contracts pallet_membership\
 pallet_multisig pallet_preimage pallet_scheduler pallet_timestamp pallet_uniques pallet_utility pallet_xcm"
export internal="pallet_allocations pallet_grants pallet_reserve"
export xcm_generic_extrinsic="report_holding, buy_execution, query_response, transact, refund_surplus,\
 set_error_handler, set_appendix, clear_error, descend_origin, clear_origin, report_error, claim_asset, trap, \
 subscribe_version, unsubscribe_version, initiate_reserve_withdraw, burn_asset, expect_asset, expect_origin,\
 expect_error, expect_transact_status, query_pallet, expect_pallet, report_transact_status,\
 clear_transact_status, set_topic, clear_topic, set_fees_mode, unpaid_execution"

cargo build --profile release \
    --features=runtime-benchmarks \
    --manifest-path=node/Cargo.toml 

for PALLET in $internal
do
./target/release/nodle-parachain benchmark pallet \
    --chain=dev \
    --steps=50 \
    --repeat=20 \
    --pallet=$PALLET \
    '--extrinsic=*' \
    --execution=wasm \
    --wasm-execution=compiled \
    --template=./.maintain/internal_pallet_weights.hbs \
    --output=runtimes/eden/src/weights
done

for PALLET in $external
do
./target/release/nodle-parachain benchmark pallet \
    --chain=dev \
    --steps=50 \
    --repeat=20 \
    --pallet=$PALLET \
    '--extrinsic=*' \
    --execution=wasm \
    --wasm-execution=compiled \
    --template=./.maintain/external_pallet_weights.hbs \
    --output=runtimes/eden/src/weights

done

./target/release/nodle-parachain benchmark pallet \
    --chain=dev \
    --steps=50 \
    --repeat=20 \
    --pallet=pallet_xcm_benchmarks::fungible \
    '--extrinsic=*' \
    --execution=wasm \
    --wasm-execution=compiled \
    --template=./.maintain/xcm.hbs \
    --output=runtimes/eden/src/weights

./target/release/nodle-parachain benchmark pallet \
    --chain=dev \
    --steps=50 \
    --repeat=20 \
    --pallet=pallet_xcm_benchmarks::generic \
    --extrinsic="$xcm_generic_extrinsic" \
    --execution=wasm \
    --wasm-execution=compiled \
    --template=./.maintain/xcm.hbs \
    --output=runtimes/eden/src/weights

echo "Running on gcloud server? Run:"
echo "    git commit -v -a -m Benchmarks ; git format-patch HEAD~"
echo "And on dev machine:"
echo "    gcloud compute scp chain-bench-012bd056:chain/0001\* . --zone=us-central1-a --tunnel-through-iap "
echo "    git apply 0001*"

