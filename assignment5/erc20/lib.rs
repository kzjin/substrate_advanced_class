#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod erc20 {
    use ink_storage::collections::HashMap as StorageHashMap;
    use ink_storage::lazy::Lazy;
    
    #[ink(storage)]
    pub struct Erc20 {
        total_supply: Lazy<Balance>,
        balances: StorageHashMap<AccountId, Balance>,
        allowances: StorageHashMap<(AccountId, AccountId), Balance>,
        issuer: AccountId
    }

    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        #[ink(topic)]
        value: Balance,
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        #[ink(topic)]
        value: Balance,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientBalance,
        InsufficientAllowance,
        InvalidIssuer,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Erc20 {
        #[ink(constructor)]
        pub fn new(total_supply: Balance) -> Self {
            let caller = Self::env().caller();
            let mut balances = StorageHashMap::new();
            balances.insert(caller, total_supply);
            let instance = Self {
                total_supply: Lazy::new(total_supply),
                balances: balances,
                allowances: StorageHashMap::new(),
                issuer: caller,
            };
            Self::env().emit_event(Transfer {
                from: None,
                to: Some(caller),
                value: total_supply,
            });
            instance
        }

        #[ink(message)]
        pub fn total_supply(&self) -> Balance {
            *self.total_supply
        }

        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> Balance {
            self.balance_of_or_zero(&owner)
        }

        #[ink(message)]
        pub fn allowance(&self, owner: AccountId, spender: AccountId) -> Balance {
            self.allowance_of_or_zero(&owner, &spender)
        }

        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, value: Balance) -> Result<()> {
            let from = self.env().caller();
            self.transfer_from_to(from, to, value)
        }

        #[ink(message)]
        pub fn approve(&mut self, spender: AccountId, value: Balance) -> Result<()> {
            let owner = self.env().caller();
            self.allowances.insert((owner, spender), value);
            self.env().emit_event(Approval {
                owner,
                spender,
                value,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> Result<()> {
            let caller = self.env().caller();
            let allowance = self.allowance_of_or_zero(&from, &caller);
            if allowance < value {
                return Err(Error::InsufficientAllowance);
            }
            self.transfer_from_to(from, to, value)?;
            self.allowances.insert((from, caller), allowance - value);
            Ok(())
        }

        /// Creates new tokens.
        #[ink(message)]
        pub fn issue(&mut self, value: Balance) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.issuer {
                return Err(Error::InvalidIssuer);
            }
            let new_total_supply = *self.total_supply + value;
            Lazy::set(&mut self.total_supply, new_total_supply);
            self.balances.insert(caller, new_total_supply);
            Self::env().emit_event(Transfer {
                from: None,
                to: Some(caller),
                value: value,
            });
            Ok(())
        }

        /// Deletes an owner's tokens.
        #[ink(message)]
        pub fn burn(&mut self, value: Balance) -> Result<()> {
            let caller = self.env().caller();
            let caller_balance = self.balance_of_or_zero(&caller);
            if caller_balance < value {
                return Err(Error::InsufficientBalance);
            }
            self.balances.insert(caller, caller_balance - value);
            let new_total_supply = *self.total_supply - value;
            Lazy::set(&mut self.total_supply, new_total_supply);
            self.balances.insert(caller, new_total_supply);
            Self::env().emit_event(Transfer {
                from: Some(caller),
                to: None,
                value: value,
            });
            Ok(())
        }

        fn transfer_from_to(
            &mut self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> Result<()> {
            let from_balance = self.balance_of_or_zero(&from);
            if from_balance < value {
                return Err(Error::InsufficientBalance);
            }
            self.balances.insert(from, from_balance - value);
            let to_balance = self.balance_of_or_zero(&to);
            self.balances.insert(to, to_balance + value);
            self.env().emit_event(Transfer {
                from: Some(from),
                to: Some(to),
                value,
            });
            Ok(())
        }

        fn balance_of_or_zero(&self, owner: &AccountId) -> Balance {
            *self.balances.get(owner).unwrap_or(&0)
        }

        fn allowance_of_or_zero(
            &self,
            owner: &AccountId,
            spender: &AccountId,
        ) -> Balance {
            *self.allowances.get(&(*owner, *spender)).unwrap_or(&0)
        }
    }

    /// Unit tests.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink_lang as ink;
        use ink_env as env;

        type Event = <Erc20 as ::ink_lang::BaseEvent>::Type;

        /// Executes the given test through the off-chain environment.
        fn run_test<F>(test_fn: F)
        where
            F: FnOnce(),
        {
            env::test::run_test::<env::DefaultEnvironment, _>(|_| {
                test_fn();
                Ok(())
            })
            .unwrap()
        }

        fn assert_transfer_event<I>(
            raw_events: I,
            transfer_index: usize,
            expected_value: u128,
        ) where
            I: IntoIterator<Item = env::test::EmittedEvent>,
        {
            let raw_event = raw_events
                .into_iter()
                .nth(transfer_index)
                .expect(&format!("No event at index {}", transfer_index));
            let event = <Event as scale::Decode>::decode(&mut &raw_event.data[..])
                .expect("Invalid contract Event");
            if let Event::Transfer(transfer) = event {
                assert_eq!(expected_value, transfer.value);
            } else {
                panic!("Expected a Transfer Event")
            }
        }

        /// The default constructor does its job.
        #[ink::test]
        fn new_works() {
            run_test(|| {
                // Constructor works.
                let _erc20 = Erc20::new(100);

                // Transfer event triggered during initial construction.
                let emitted_events = env::test::recorded_events().collect::<Vec<_>>();
                assert_eq!(1, emitted_events.len());

                assert_transfer_event(emitted_events, 0, 100)
            })
        }

        /// The total supply was applied.
        #[ink::test]
        fn total_supply_works() {
            run_test(|| {
                // Constructor works.
                let erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                // Get the token total supply.
                assert_eq!(erc20.total_supply(), 100);
            })
        }

        /// Get the actual balance of an account.
        #[ink::test]
        fn balance_of_works() {
            run_test(|| {
                // Constructor works
                let erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");
                // Alice owns all the tokens on deployment
                assert_eq!(erc20.balance_of(accounts.alice), 100);
                // Bob does not owns tokens
                assert_eq!(erc20.balance_of(accounts.bob), 0);
            })
        }

        #[ink::test]
        fn transfer_works() {
            run_test(|| {
                // Constructor works.
                let mut erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");

                assert_eq!(erc20.balance_of(accounts.bob), 0);
                // Alice transfers 10 tokens to Bob.
                assert_eq!(erc20.transfer(accounts.bob, 10), Ok(()));
                // The second Transfer event takes place.
                assert_transfer_event(env::test::recorded_events(), 1, 10);
                // Bob owns 10 tokens.
                assert_eq!(erc20.balance_of(accounts.bob), 10);
            })
        }

        #[ink::test]
        fn invalid_transfer_should_fail() {
            run_test(|| {
                // Constructor works.
                let mut erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");

                assert_eq!(erc20.balance_of(accounts.bob), 0);
                // Get contract address.
                let callee =
                    env::account_id::<env::DefaultEnvironment>().unwrap_or([0x0; 32].into());
                // Create call
                let mut data =
                    env::test::CallData::new(env::call::Selector::new([0x00; 4])); // balance_of
                data.push_arg(&accounts.bob);
                // Push the new execution context to set Bob as caller
                assert_eq!(
                    env::test::push_execution_context::<env::DefaultEnvironment>(
                        accounts.bob,
                        callee,
                        1000000,
                        1000000,
                        data
                    ),
                    ()
                );

                // Bob fails to transfers 10 tokens to Eve.
                assert_eq!(erc20.transfer(accounts.eve, 10), Err(Error::InsufficientBalance));
                // Alice owns all the tokens.
                assert_eq!(erc20.balance_of(accounts.alice), 100);
                assert_eq!(erc20.balance_of(accounts.bob), 0);
                assert_eq!(erc20.balance_of(accounts.eve), 0);
            })
        }

        #[ink::test]
        fn issue_works() {
            run_test(|| {
                // Constructor works.
                let mut erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");

                // Alice owns all the tokens on deployment
                assert_eq!(erc20.balance_of(accounts.alice), 100);

                // issue 10 more tokens
                assert_eq!(erc20.issue(10), Ok(()));

                // Alice owns all the tokens on deployment
                assert_eq!(erc20.balance_of(accounts.alice), 110);
            })
        }

        #[ink::test]
        fn burn_works() {
            run_test(|| {
                // Constructor works.
                let mut erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");

                // Alice owns all the tokens on deployment
                assert_eq!(erc20.balance_of(accounts.alice), 100);
                // burn 10 tokens
                assert_eq!(erc20.burn(10), Ok(()));

                // Alice owns all the tokens on deployment
                assert_eq!(erc20.balance_of(accounts.alice), 90);
            })
        }

        #[ink::test]
        fn transfer_from_works() {
            run_test(|| {
                // Constructor works.
                let mut erc20 = Erc20::new(100);
                // Transfer event triggered during initial construction.
                assert_transfer_event(env::test::recorded_events(), 0, 100);
                let accounts = env::test::default_accounts::<env::DefaultEnvironment>()
                    .expect("Cannot get accounts");

                // Bob fails to transfer tokens owned by Alice.
                assert_eq!(erc20.transfer_from(accounts.alice, accounts.eve, 10), Err(Error::InsufficientAllowance));
                // Alice approves Bob for token transfers on her behalf.
                assert_eq!(erc20.approve(accounts.bob, 10), Ok(()));

                // The approve event takes place.
                assert_eq!(env::test::recorded_events().count(), 2);

                // Get contract address.
                let callee =
                    env::account_id::<env::DefaultEnvironment>().unwrap_or([0x0; 32].into());
                // Create call.
                let mut data =
                    env::test::CallData::new(env::call::Selector::new([0x00; 4])); // balance_of
                data.push_arg(&accounts.bob);
                // Push the new execution context to set Bob as caller.
                assert_eq!(
                    env::test::push_execution_context::<env::DefaultEnvironment>(
                        accounts.bob,
                        callee,
                        1000000,
                        1000000,
                        data
                    ),
                    ()
                );

                // Bob transfers tokens from Alice to Eve.
                assert_eq!(erc20.transfer_from(accounts.alice, accounts.eve, 10), Ok(()));
                // The third event takes place.
                assert_transfer_event(env::test::recorded_events(), 2, 10);
                // Eve owns tokens.
                assert_eq!(erc20.balance_of(accounts.eve), 10);
            })
        }
    }
}
