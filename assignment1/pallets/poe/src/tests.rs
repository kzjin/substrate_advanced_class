use crate::{Error, mock::*};
use frame_support::{assert_ok, assert_noop};

use super::*;

#[test]
fn create_claim_works() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));
        assert_eq!(Proofs::<Test>::get(&claim), (1, frame_system::Module::<Test>::block_number()));
    });
}

#[test]
fn create_claim_fails_when_claim_already_exists() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));

        assert_noop!(
            PoeModule::create_claim(Origin::signed(1), claim.clone()),
            Error::<Test>::ProofAlreadyClaimed
        );
    });
}

#[test]
fn create_claim_fails_when_proof_is_too_long() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0; 1024];
        assert_noop!(
            PoeModule::create_claim(Origin::signed(1), claim.clone()),
            Error::<Test>::ProofTooLong
        );
    });
}

#[test]
fn revoke_claim_works() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));
        assert_ok!(PoeModule::revoke_claim(Origin::signed(1), claim.clone()));
    });
}

#[test]
fn revoke_claim_fails_when_claim_does_not_exist() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_noop!(
            PoeModule::revoke_claim(Origin::signed(1), claim.clone()),
            Error::<Test>::NoSuchProof
        );
    });
}

#[test]
fn revoke_claim_fails_when_request_by_not_owner() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));
        assert_noop!(
            PoeModule::revoke_claim(Origin::signed(2), claim.clone()),
            Error::<Test>::NotProofOwner
        );
    });
}

#[test]
fn transfer_claim_works() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));
        assert_ok!(PoeModule::transfer_claim(Origin::signed(1), claim.clone(), 2));
        assert_eq!(Proofs::<Test>::get(&claim), (2, frame_system::Module::<Test>::block_number()));
    });
}

#[test]
fn transfer_claim_fails_when_claim_does_not_exist() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_noop!(
            PoeModule::transfer_claim(Origin::signed(1), claim.clone(), 2),
            Error::<Test>::NoSuchProof
        );
    });
}

#[test]
fn transfer_claim_fails_when_request_by_not_owner() {
    new_test_ext().execute_with(|| {
        let claim: Vec<u8> = vec![0, 2];
        assert_ok!(PoeModule::create_claim(Origin::signed(1), claim.clone()));
        assert_noop!(
            PoeModule::transfer_claim(Origin::signed(2), claim.clone(), 3),
            Error::<Test>::NotProofOwner
        );
    });
}
