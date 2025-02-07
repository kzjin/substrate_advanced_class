#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter};
use frame_support::traits::{Currency, ReservableCurrency, Randomness, Get, Vec};
use sp_io::hashing::blake2_128;
use frame_system::ensure_signed;
use sp_runtime::{DispatchError, DispatchResult};
use sp_runtime::traits::{AtLeast32Bit, Bounded, Member};
use sp_std::convert::TryInto;

#[derive(Encode, Decode)]
pub struct Kitty(pub [u8; 16]);

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

pub trait Trait: frame_system::Trait {
    /// Because this pallet emits events, it depends on the runtime's definition of an event.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type Randomness: Randomness<Self::Hash>;
    type KittyIndex: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy + TryInto<u32>;
    type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
    type DepositValue: Get<u32>;
}

decl_storage! {
    trait Store for Module<T: Trait> as Kitties {
        /// Stores all the kitties, key is the kitty id / index
        pub Kitties get(fn kitties): map hasher(blake2_128_concat) T::KittyIndex => Option<Kitty>;
        /// Stores the total number of kitties. i.e. the next kitty index
        pub KittiesCount get(fn kitties_count): T::KittyIndex;
        /// Get the kitty owner by kitty id
        pub KittyOwner get(fn kitty_owner): map hasher(blake2_128_concat) T::KittyIndex => Option<T::AccountId>;
        /// Get kitties owned by account ID
        pub OwnedKitties get(fn owned_kitties): map hasher(blake2_128_concat) T::AccountId => Vec<T::KittyIndex>;
        /// Get parent IDs by kitty index
        pub KittyParents get(fn kitty_parents): map hasher(blake2_128_concat) T::KittyIndex => Option<(T::KittyIndex, T::KittyIndex)>;
        /// Get sibling IDs by kitty index
        pub KittySiblings get(fn kitty_siblings): map hasher(blake2_128_concat) T::KittyIndex => Vec<T::KittyIndex>;
        /// Get child IDs by kitty index
        pub KittyChildren get(fn kitty_children): map hasher(blake2_128_concat) T::KittyIndex => Vec<T::KittyIndex>;
        /// Get partner ID by kitty index
        pub KittyPartners get(fn kitty_partners): map hasher(blake2_128_concat) T::KittyIndex => Vec<T::KittyIndex>;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        KittiesCountOverflow,
        InvalidKittyId,
        RequireDifferentParent,
        NotValidOwner,
        NotValidReceiver,
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId, KittyIndex = <T as Trait>::KittyIndex {
        /// Event emitted when a kitty is created. [who, index]
        Created(AccountId, KittyIndex),
        /// Event emitted when a kitty is transferred. [from, to, index]
        Transferred(AccountId, AccountId, KittyIndex),
        /// Event emitted when a kitty is born. [who, idx1, idx2, new_idx]
        Breeded(AccountId, KittyIndex, KittyIndex, KittyIndex),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        // Events must be initialized if they are used by the pallet.
        fn deposit_event() = default;

        /// Create a new kitty
        #[weight = 0]
        pub fn create(origin) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let kitty_id = Self::next_kitty_id()?;
            // Generate a random 128bit value
            let dna = Self::random_value(&sender);
            // Create and store kitty
            let kitty = Kitty(dna);

            Self::insert_kitty(&sender, kitty_id, kitty)?;
            Self::deposit_event(RawEvent::Created(sender, kitty_id));

            Ok(())
        }

        /// transfer a kitty
        #[weight = 0]
        pub fn transfer(origin, to: T::AccountId, kitty_id: T::KittyIndex) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let owner = Self::kitty_owner(kitty_id).ok_or(Error::<T>::InvalidKittyId)?;
            // !!!
            // problem 1: didn't check if "sender" is the owner of the kitty with id "kitty_id"
            // !!!
            ensure!(sender == owner, Error::<T>::NotValidOwner);
            ensure!(sender != to, Error::<T>::NotValidReceiver);

            Self::remove_kitty_from_owner(&sender, kitty_id)?;
            Self::add_kitty_to_owner(&to, kitty_id)?;
            <KittyOwner<T>>::insert(kitty_id, to.clone());
            Self::deposit_event(RawEvent::Transferred(sender, to, kitty_id));

            Ok(())
        }

        /// Breed kitties
        #[weight = 0]
        pub fn breed(origin, kitty_id_1: T::KittyIndex, kitty_id_2: T::KittyIndex) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            Self::do_breed(sender, kitty_id_1, kitty_id_2)?;

            Ok(())
        }
    }
}

fn combine_dna(dna1: u8, dna2: u8, selector: u8) -> u8 {
    (selector & dna1) | (!selector & dna2)
}

impl<T: Trait> Module<T> {
    fn random_value(sender: &T::AccountId) -> [u8; 16] {
        let payload = (
            T::Randomness::random_seed(),
            &sender,
            <frame_system::Module<T>>::extrinsic_index(),
        );
        payload.using_encoded(blake2_128)
    }

    fn next_kitty_id() -> sp_std::result::Result<T::KittyIndex, DispatchError> {
        let kitty_id = Self::kitties_count();
        if kitty_id == T::KittyIndex::max_value() {
            return Err(Error::<T>::KittiesCountOverflow.into());
        }
        Ok(kitty_id)
    }

    fn insert_kitty(owner: &T::AccountId, kitty_id: T::KittyIndex, kitty: Kitty) -> DispatchResult {
        <Kitties<T>>::insert(kitty_id, kitty);
        <KittiesCount<T>>::put(kitty_id + 1.into());
        <KittyOwner<T>>::insert(kitty_id, owner);
        Self::add_kitty_to_owner(&owner, kitty_id)?;

        Ok(())
    }

    fn add_kitty_to_owner(owner: &T::AccountId, kitty_id: T::KittyIndex) -> DispatchResult {
        T::Currency::reserve(&owner, BalanceOf::<T>::from(T::DepositValue::get()))?;
        let mut kitty_list = <OwnedKitties<T>>::get(&owner);
        kitty_list.push(kitty_id);
        <OwnedKitties<T>>::insert(&owner, kitty_list);

        Ok(())
    }

    fn remove_kitty_from_owner(owner: &T::AccountId, kitty_id: T::KittyIndex) -> DispatchResult {
        let mut kitty_list = <OwnedKitties<T>>::get(&owner);
        if let Some(index) = kitty_list.iter().position(|x| *x == kitty_id) {
            T::Currency::unreserve(&owner, BalanceOf::<T>::from(T::DepositValue::get()));
            kitty_list.remove(index);
            <OwnedKitties<T>>::insert(owner, kitty_list);
        }
        Ok(())
    }

    fn do_breed(sender: T::AccountId, kitty_id_1: T::KittyIndex, kitty_id_2: T::KittyIndex) -> sp_std::result::Result<T::KittyIndex, DispatchError> {
        let kitty1 = Self::kitties(kitty_id_1).ok_or(Error::<T>::InvalidKittyId)?;
        let kitty2 = Self::kitties(kitty_id_2).ok_or(Error::<T>::InvalidKittyId)?;

        ensure!(kitty_id_1 != kitty_id_2, Error::<T>::RequireDifferentParent);

        let kitty_id = Self::next_kitty_id()?;

        let kitty1_dna = kitty1.0;
        let kitty2_dna = kitty2.0;

        // Generate a random 128bit value
        let selector = Self::random_value(&sender);
        let mut new_dna = [0u8; 16];

        // Combine parents and selector to create new kitty
        for i in 0..kitty1_dna.len() {
            new_dna[i] = combine_dna(kitty1_dna[i], kitty2_dna[i], selector[i]);
        }

        Self::insert_kitty(&sender, kitty_id, Kitty(new_dna))?;

        // parents
        // time: O(1); space: O(1)
        <KittyParents<T>>::insert(kitty_id, (kitty_id_1, kitty_id_2));

        // partners
        // time: O(n); space: O(n)
        let mut partner_list = <KittyPartners<T>>::get(&kitty_id_1);
        let mut prev_partner = false;
        for item in &partner_list {
            if *item == kitty_id_2 {
                prev_partner = true;
                break;
            }
        }
        if prev_partner == false {
            partner_list.push(kitty_id_2);
            <KittyPartners<T>>::insert(kitty_id_1, partner_list);
            partner_list = <KittyPartners<T>>::get(&kitty_id_2);
            partner_list.push(kitty_id_1);
            <KittyPartners<T>>::insert(kitty_id_2, partner_list);
        }

        // siblings: ignore the potential duplicate kitties added to sibling list here
        // time: O(n); space: O(n)
        let mut new_sibling_list = <KittySiblings<T>>::get(&kitty_id);
        let mut child_list = <KittyChildren<T>>::get(&kitty_id_1);
        for item in &child_list {
            let mut sibling_list = <KittySiblings<T>>::get(item);
            sibling_list.push(kitty_id);
            <KittyPartners<T>>::insert(item, sibling_list);
            new_sibling_list.push(*item);
        }
        child_list = <KittyChildren<T>>::get(&kitty_id_2);
        for item in &child_list {
            let mut sibling_list = <KittySiblings<T>>::get(item);
            sibling_list.push(kitty_id);
            <KittyPartners<T>>::insert(item, sibling_list);
            new_sibling_list.push(*item);
        }
        <KittySiblings<T>>::insert(kitty_id, new_sibling_list);

        // children
        // time: O(n); space: O(n)
        <KittyChildren<T>>::mutate(&kitty_id_1, |val| val.push(kitty_id));
        <KittyChildren<T>>::mutate(&kitty_id_2, |val| val.push(kitty_id));

        Self::deposit_event(RawEvent::Breeded(sender, kitty_id_1, kitty_id_2, kitty_id));

        Ok(kitty_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Module, Trait};
    use sp_core::H256;
    use frame_support::{impl_outer_origin, parameter_types, weights::Weight,
        traits::{OnFinalize, OnInitialize}};
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup}, testing::Header, Perbill,
    };
    use frame_support::{assert_noop};
    use frame_system as system;

    pub(crate) type Balance = u128;

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    parameter_types! {
    pub const BlockHashCount: u64 = 250;
        pub const MaximumBlockWeight: Weight = 1024;
        pub const MaximumBlockLength: u32 = 2 * 1024;
        pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    }

    impl system::Trait for Test {
        type BaseCallFilter = ();
        type Origin = Origin;
        type Call = ();
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type BlockHashCount = BlockHashCount;
        type MaximumBlockWeight = MaximumBlockWeight;
        type DbWeight = ();
        type BlockExecutionWeight = ();
        type ExtrinsicBaseWeight = ();
        type MaximumExtrinsicWeight = MaximumBlockWeight;
        type MaximumBlockLength = MaximumBlockLength;
        type AvailableBlockRatio = AvailableBlockRatio;
        type Version = ();
        type PalletInfo = ();
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
    }

    parameter_types! {
        pub const ExistentialDeposit: Balance = 100;
        // For weight estimation, we assume that the most locks on an individual account will be 50.
        // This number may need to be adjusted in the future if this assumption no longer holds true.
        pub const MaxLocks: u32 = 50;
    }

    impl pallet_balances::Trait for Test {
	type MaxLocks = ();
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = ();
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
    }

    pub type Randomness = pallet_randomness_collective_flip::Module<Test>;
    pub type System = frame_system::Module<Test>;
    pub type Balances = pallet_balances::Module<Test>;

    impl Trait for Test {
        type Event = ();
        type Randomness = Randomness;
	type KittyIndex = u32;
	type Currency = Balances;
	type DepositValue = DepositValue;
    }

    parameter_types! {
	pub const DepositValue: u32 = 10;
    }

    pub type Kitties = Module<Test>;

    fn run_to_block(n: u64) {
        while System::block_number() < n {
            Kitties::on_finalize(System::block_number());
            System::on_finalize(System::block_number());
            System::set_block_number(System::block_number() + 1);
            System::on_initialize(System::block_number());
            Kitties::on_initialize(System::block_number());
        }
    }

    pub fn new_test_ext() -> sp_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        let _ = pallet_balances::GenesisConfig::<Test> {
            balances: vec![(1, 500), (2, 500)],
        }
        .assimilate_storage(&mut storage);

        let ext = sp_io::TestExternalities::from(storage);
        ext
    }

    #[test]
    fn kitty_create_works() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
        });
    }

    #[test]
    fn kitty_transfer_works() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_eq!(Kitties::transfer(Origin::signed(1), 2, 0), Ok(()));
        });
    }

    #[test]
    fn kitty_transfer_fails_not_valid_kitty() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_noop!(
                Kitties::transfer(Origin::signed(1), 3, 2),
                Error::<Test>::InvalidKittyId
            );
        });
    }

    #[test]
    fn kitty_transfer_fails_not_valid_owner() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_noop!(
                Kitties::transfer(Origin::signed(2), 3, 0),
                Error::<Test>::NotValidOwner
            );
        });
    }

    #[test]
    fn kitty_transfer_fails_not_valid_receiver() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_noop!(
                Kitties::transfer(Origin::signed(1), 1, 0),
                Error::<Test>::NotValidReceiver
            );
        });
    }

    #[test]
    fn kitty_breed_works() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_eq!(Kitties::breed(Origin::signed(1), 0, 1), Ok(()));
        });
    }

    #[test]
    fn kitty_breed_fails_not_valid_kitty() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_noop!(
                Kitties::breed(Origin::signed(1), 3, 0),
                Error::<Test>::InvalidKittyId
            );
        });
    }

    #[test]
    fn kitty_breed_fails_same_parents() {
        new_test_ext().execute_with(|| {
            run_to_block(10);
            assert_eq!(Kitties::create(Origin::signed(1)), Ok(()));
            assert_noop!(
                Kitties::breed(Origin::signed(1), 0, 0),
                Error::<Test>::RequireDifferentParent
            );
        });
    }
}
