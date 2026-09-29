use std::collections::BTreeMap;

use frost_core as frost;
use frost_secp256k1 as frostk;
use rand_core::OsRng;

const MAX_SIGNERS: u16 = 5;
const MIN_SIGNERS: u16 = 3;

type Id = frostk::Identifier;

fn main() {
    let mut rng = OsRng;
    let n = MAX_SIGNERS as usize;
    let t = MIN_SIGNERS as usize;

    // ---- DKG round 1 ----
    let mut r1_secret = BTreeMap::<Id, frostk::keys::dkg::round1::SecretPackage>::new();
    let mut r1_recv = BTreeMap::<Id, BTreeMap<Id, frostk::keys::dkg::round1::Package>>::new();
    for i in 1..=n {
        let id: Id = (i as u16).try_into().unwrap();
        let (secret, package) = frostk::keys::dkg::part1(id, MAX_SIGNERS, MIN_SIGNERS, &mut rng).unwrap();
        r1_secret.insert(id, secret);
        for j in 1..=n {
            if i == j {
                continue;
            }
            let rid: Id = (j as u16).try_into().unwrap();
            r1_recv.entry(rid).or_default().insert(id, package.clone());
        }
    }

    // ---- DKG round 2 ----
    let mut r2_secret = BTreeMap::<Id, frostk::keys::dkg::round2::SecretPackage>::new();
    let mut r2_recv = BTreeMap::<Id, BTreeMap<Id, frostk::keys::dkg::round2::Package>>::new();
    for i in 1..=n {
        let id: Id = (i as u16).try_into().unwrap();
        let (secret, packages) =
            frostk::keys::dkg::part2(r1_secret.remove(&id).unwrap(), &r1_recv[&id]).unwrap();
        r2_secret.insert(id, secret);
        for (receiver, package) in packages {
            r2_recv.entry(receiver).or_default().insert(id, package);
        }
    }

    // ---- DKG part 3: long-lived key packages ----
    let mut key_packages = BTreeMap::<Id, frostk::keys::KeyPackage>::new();
    let mut pubkey_package = None;
    for i in 1..=n {
        let id: Id = (i as u16).try_into().unwrap();
        let (key_package, pkgs) =
            frostk::keys::dkg::part3(&r2_secret[&id], &r1_recv[&id], &r2_recv[&id]).unwrap();
        pubkey_package = Some(pkgs);
        key_packages.insert(id, key_package);
    }
    let pubkey_package = pubkey_package.unwrap();

    let group_vk = pubkey_package.verifying_key().serialize().unwrap();
    println!("trustees              {n}");
    println!("threshold             {t}-of-{n}");
    println!("group verifying key   {}", hex::encode(&group_vk));

    let message = b"inheritance-release-attestation";

    // ---- signing round 1: a {t}-signer subset commits ----
    let signers: Vec<Id> = (1..=t).map(|i| (i as u16).try_into().unwrap()).collect();
    let mut nonces = BTreeMap::new();
    let mut commitments = BTreeMap::<Id, frostk::round1::SigningCommitments>::new();
    for id in &signers {
        let kp = &key_packages[id];
        let (nonce, commitment) = frostk::round1::commit(&kp.signing_share(), &mut rng);
        nonces.insert(*id, nonce);
        commitments.insert(*id, commitment);
    }

    let signing_package = frostk::SigningPackage::new(commitments, message);

    // ---- signing round 2: each signer produces a share ----
    let mut shares = BTreeMap::new();
    for id in &signers {
        let kp = &key_packages[id];
        let share = frostk::round2::sign(&signing_package, &nonces[id], kp).unwrap();
        frost::verify_signature_share(
            *id,
            &kp.verifying_share(),
            &share,
            &signing_package,
            &pubkey_package.verifying_key(),
        )
        .unwrap();
        shares.insert(*id, share);
    }

    let signature = frostk::aggregate(&signing_package, &shares, &pubkey_package).unwrap();
    pubkey_package.verifying_key().verify(message, &signature).unwrap();

    println!();
    println!("message               {}", String::from_utf8_lossy(message));
    println!("shares aggregated     {}/{} signers", shares.len(), n);
    println!("frost signature       {}", hex::encode(signature.serialize().unwrap()));
    println!("verified against vkey ok");

    // ---- a subset below the threshold must be unable to sign ----
    let short: Vec<Id> = (1..t).map(|i| (i as u16).try_into().unwrap()).collect();
    let mut short_commitments = BTreeMap::<Id, frostk::round1::SigningCommitments>::new();
    let mut short_nonces = BTreeMap::new();
    for id in &short {
        let kp = &key_packages[id];
        let (nonce, commitment) = frostk::round1::commit(&kp.signing_share(), &mut rng);
        short_nonces.insert(*id, nonce);
        short_commitments.insert(*id, commitment);
    }
    let short_package = frostk::SigningPackage::new(short_commitments, message);
    let mut short_shares = BTreeMap::new();
    for id in &short {
        let kp = &key_packages[id];
        match frostk::round2::sign(&short_package, &short_nonces[id], kp) {
            Ok(share) => {
                short_shares.insert(*id, share);
            }
            Err(e) => println!("below-threshold sign   rejected: {e}"),
        }
    }
    match frostk::aggregate(&short_package, &short_shares, &pubkey_package) {
        Ok(_) => println!("below-threshold aggregate UNEXPECTEDLY SUCCEEDED"),
        Err(e) => println!("below-threshold aggregate rejected: {e}"),
    }
}
