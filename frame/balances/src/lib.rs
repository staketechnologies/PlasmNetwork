// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! # Balances Module
//!
//! The Balances module provides functionality for handling accounts and balances.
//!
//! - [`balances::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//! - [`Module`](./struct.Module.html)
//!
//! ## Overview
//!
//! The Balances module provides functions for:
//!
//! - Getting and setting free balances.
//! - Retrieving total, reserved and unreserved balances.
//! - Repatriating a reserved balance to a beneficiary account that exists.
//! - Transferring a balance between accounts (when not reserved).
//! - Slashing an account balance.
//! - Account creation and removal.
//! - Managing total issuance.
//! - Setting and managing locks.
//!
//! ### Terminology
//!
//! - **Existential Deposit:** The minimum balance required to create or keep an account open. This prevents
//! "dust accounts" from filling storage. When the free plus the reserved balance (i.e. the total balance)
//!   fall below this, then the account is said to be dead; and it loses its functionality as well as any
//!   prior history and all information on it is removed from the chain's state.
//!   No account should ever have a total balance that is strictly between 0 and the existential
//!   deposit (exclusive). If this ever happens, it indicates either a bug in this module or an
//!   erroneous raw mutation of storage.
//!
//! - **Total Issuance:** The total number of units in existence in a system.
//!
//! - **Reaping an account:** The act of removing an account by resetting its nonce. Happens after its
//! total balance has become zero (or, strictly speaking, less than the Existential Deposit).
//!
//! - **Free Balance:** The portion of a balance that is not reserved. The free balance is the only
//!   balance that matters for most operations.
//!
//! - **Reserved Balance:** Reserved balance still belongs to the account holder, but is suspended.
//!   Reserved balance can still be slashed, but only after all the free balance has been slashed.
//!
//! - **Imbalance:** A condition when some funds were credited or debited without equal and opposite accounting
//! (i.e. a difference between total issuance and account balances). Functions that result in an imbalance will
//! return an object of the `Imbalance` trait that can be managed within your runtime logic. (If an imbalance is
//! simply dropped, it should automatically maintain any book-keeping such as total issuance.)
//!
//! - **Lock:** A freeze on a specified amount of an account's free balance until a specified block number. Multiple
//! locks always operate over the same funds, so they "overlay" rather than "stack".
//!
//! ### Implementations
//!
//! The Balances module provides implementations for the following traits. If these traits provide the functionality
//! that you need, then you can avoid coupling with the Balances module.
//!
//! - [`Currency`](../frame_support/traits/trait.Currency.html): Functions for dealing with a
//! fungible assets system.
//! - [`ReservableCurrency`](../frame_support/traits/trait.ReservableCurrency.html):
//! Functions for dealing with assets that can be reserved from an account.
//! - [`LockableCurrency`](../frame_support/traits/trait.LockableCurrency.html): Functions for
//! dealing with accounts that allow liquidity restrictions.
//! - [`Imbalance`](../frame_support/traits/trait.Imbalance.html): Functions for handling
//! imbalances between total issuance in the system and account balances. Must be used when a function
//! creates new funds (e.g. a reward) or destroys some funds (e.g. a system fee).
//! - [`IsDeadAccount`](../frame_system/trait.IsDeadAccount.html): Determiner to say whether a
//! given account is unused.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `transfer` - Transfer some liquid free balance to another account.
//! - `set_balance` - Set the balances of a given account. The origin of this call must be root.
//!
//! ## Usage
//!
//! The following examples show how to use the Balances module in your custom module.
//!
//! ### Examples from the FRAME
//!
//! The Contract module uses the `Currency` trait to handle gas payment, and its types inherit from `Currency`:
//!
//! ```
//! use frame_support::traits::Currency;
//! # pub trait Trait: frame_system::Trait {
//! # 	type Currency: Currency<Self::AccountId>;
//! # }
//!
//! pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
//! pub type NegativeImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;
//!
//! # fn main() {}
//! ```
//!
//! The Staking module uses the `LockableCurrency` trait to lock a stash account's funds:
//!
//! ```
//! use frame_support::traits::{WithdrawReasons, LockableCurrency};
//! use sp_runtime::traits::Bounded;
//! pub trait Trait: frame_system::Trait {
//! 	type Currency: LockableCurrency<Self::AccountId, Moment=Self::BlockNumber>;
//! }
//! # struct StakingLedger<T: Trait> {
//! # 	stash: <T as frame_system::Trait>::AccountId,
//! # 	total: <<T as Trait>::Currency as frame_support::traits::Currency<<T as frame_system::Trait>::AccountId>>::Balance,
//! # 	phantom: std::marker::PhantomData<T>,
//! # }
//! # const STAKING_ID: [u8; 8] = *b"staking ";
//!
//! fn update_ledger<T: Trait>(
//! 	controller: &T::AccountId,
//! 	ledger: &StakingLedger<T>
//! ) {
//! 	T::Currency::set_lock(
//! 		STAKING_ID,
//! 		&ledger.stash,
//! 		ledger.total,
//! 		WithdrawReasons::all()
//! 	);
//! 	// <Ledger<T>>::insert(controller, ledger); // Commented out as we don't have access to Staking's storage here.
//! }
//! # fn main() {}
//! ```
//!
//! ## Genesis config
//!
//! The Balances module depends on the [`GenesisConfig`](./struct.GenesisConfig.html).
//!
//! ## Assumptions
//!
//! * Total issued balanced of all accounts should be less than `Trait::Balance::max_value()`.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
mod tests;
mod tests_local;
mod tests_composite;
mod benchmarking;

use sp_std::prelude::*;
use sp_std::{cmp, result, mem, fmt::Debug, ops::BitOr, convert::Infallible};
use codec::{Codec, Encode, Decode};
use frame_support::{
	StorageValue, Parameter, decl_event, decl_storage, decl_module, decl_error, ensure,
	weights::SimpleDispatchInfo, traits::{
		Currency, OnKilledAccount, OnUnbalanced, TryDrop, StoredMap,
		WithdrawReason, WithdrawReasons, LockIdentifier, LockableCurrency, ExistenceRequirement,
		Imbalance, SignedImbalance, ReservableCurrency, Get, ExistenceRequirement::KeepAlive,
		ExistenceRequirement::AllowDeath, IsDeadAccount, BalanceStatus as Status,
	}
};
use sp_runtime::{
	RuntimeDebug, DispatchResult, DispatchError,
	traits::{
		Zero, AtLeast32Bit, StaticLookup, Member, CheckedAdd, CheckedSub,
		MaybeSerializeDeserialize, Saturating, Bounded,
	},
};
use frame_system::{self as system, ensure_signed, ensure_root};

pub use self::imbalances::{PositiveImbalance, NegativeImbalance};

pub trait Subtrait<I: Instance = DefaultInstance>: frame_system::Trait {
	/// The balance of an account.
	type Balance: Parameter + Member + AtLeast32Bit + Codec + Default + Copy +
		MaybeSerializeDeserialize + Debug;

	/// The minimum amount required to keep an account open.
	type ExistentialDeposit: Get<Self::Balance>;

	/// The means of storing the balances of an account.
	type AccountStore: StoredMap<Self::AccountId, AccountData<Self::Balance>>;
}

pub trait Trait<I: Instance = DefaultInstance>: frame_system::Trait {
	/// The balance of an account.
	type Balance: Parameter + Member + AtLeast32Bit + Codec + Default + Copy +
		MaybeSerializeDeserialize + Debug;

	/// Handler for the unbalanced reduction when removing a dust account.
	type DustRemoval: OnUnbalanced<NegativeImbalance<Self, I>>;

	/// The overarching event type.
	type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;

	/// The minimum amount required to keep an account open.
	type ExistentialDeposit: Get<Self::Balance>;

	/// The means of storing the balances of an account.
	type AccountStore: StoredMap<Self::AccountId, AccountData<Self::Balance>>;
}

impl<T: Trait<I>, I: Instance> Subtrait<I> for T {
	type Balance = T::Balance;
	type ExistentialDeposit = T::ExistentialDeposit;
	type AccountStore = T::AccountStore;
}

decl_event!(
	pub enum Event<T, I: Instance = DefaultInstance> where
		<T as frame_system::Trait>::AccountId,
		<T as Trait<I>>::Balance
	{
		/// An account was created with some free balance.
		Endowed(AccountId, Balance),
		/// An account was removed whose balance was non-zero but below ExistentialDeposit,
		/// resulting in an outright loss.
		DustLost(AccountId, Balance),
		/// Transfer succeeded (from, to, value).
		Transfer(AccountId, AccountId, Balance),
		/// A balance was set by root (who, free, reserved).
		BalanceSet(AccountId, Balance, Balance),
		/// Some amount was deposited (e.g. for transaction fees).
		Deposit(AccountId, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait<I>, I: Instance> {
		/// Vesting balance too high to send value
		VestingBalance,
		/// Account liquidity restrictions prevent withdrawal
		LiquidityRestrictions,
		/// Got an overflow after adding
		Overflow,
		/// Balance too low to send value
		InsufficientBalance,
		/// Value too low to create account due to existential deposit
		ExistentialDeposit,
		/// Transfer/payment would kill account
		KeepAlive,
		/// A vesting schedule already exists for this account
		ExistingVestingSchedule,
		/// Beneficiary account must pre-exist
		DeadAccount,
	}
}

/// Simplified reasons for withdrawing balance.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum Reasons {
	/// Paying system transaction fees.
	Fee = 0,
	/// Any reason other than paying system transaction fees.
	Misc = 1,
	/// Any reason at all.
	All = 2,
}

impl From<WithdrawReasons> for Reasons {
	fn from(r: WithdrawReasons) -> Reasons {
		if r == WithdrawReasons::from(WithdrawReason::TransactionPayment) {
			Reasons::Fee
		} else if r.contains(WithdrawReason::TransactionPayment) {
			Reasons::All
		} else {
			Reasons::Misc
		}
	}
}

impl BitOr for Reasons {
	type Output = Reasons;
	fn bitor(self, other: Reasons) -> Reasons {
		if self == other { return self }
		Reasons::All
	}
}

/// A single lock on a balance. There can be many of these on an account and they "overlap", so the
/// same balance is frozen by multiple locks.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BalanceLock<Balance> {
	/// An identifier for this lock. Only one lock may be in existence for each identifier.
	pub id: LockIdentifier,
	/// The amount which the free balance may not drop below when this lock is in effect.
	pub amount: Balance,
	/// If true, then the lock remains in effect even for payment of transaction fees.
	pub reasons: Reasons,
}

/// All balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct AccountData<Balance> {
	/// Non-reserved part of the balance. There may still be restrictions on this, but it is the
	/// total pool what may in principle be transferred, reserved and used for tipping.
	///
	/// This is the only balance that matters in terms of most operations on tokens. It
	/// alone is used to determine the balance when in the contract execution environment.
	pub free: Balance,
	/// Balance which is reserved and may not be used at all.
	///
	/// This can still get slashed, but gets slashed last of all.
	///
	/// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
	/// that are still 'owned' by the account holder, but which are suspendable.
	pub reserved: Balance,
	/// The amount that `free` may not drop below when withdrawing for *anything except transaction
	/// fee payment*.
	pub misc_frozen: Balance,
	/// The amount that `free` may not drop below when withdrawing specifically for transaction
	/// fee payment.
	pub fee_frozen: Balance,
}

impl<Balance: Saturating + Copy + Ord> AccountData<Balance> {
	/// How much this account's balance can be reduced for the given `reasons`.
	fn usable(&self, reasons: Reasons) -> Balance {
		self.free.saturating_sub(self.frozen(reasons))
	}
	/// The amount that this account's free balance may not be reduced beyond for the given
	/// `reasons`.
	fn frozen(&self, reasons: Reasons) -> Balance {
		match reasons {
			Reasons::All => self.misc_frozen.max(self.fee_frozen),
			Reasons::Misc => self.misc_frozen,
			Reasons::Fee => self.fee_frozen,
		}
	}
	/// The total balance in this account including any that is reserved and ignoring any frozen.
	fn total(&self) -> Balance {
		self.free.saturating_add(self.reserved)
	}
}

// A value placed in storage that represents the current version of the Balances storage.
// This value is used by the `on_runtime_upgrade` logic to determine whether we run
// storage migration logic. This should match directly with the semantic versions of the Rust crate.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
enum Releases {
	V1_0_0,
	V2_0_0,
}

impl Default for Releases {
	fn default() -> Self {
		Releases::V1_0_0
	}
}

decl_storage! {
	trait Store for Module<T: Trait<I>, I: Instance=DefaultInstance> as Balances {
		/// The total units issued in the system.
		pub TotalIssuance get(fn total_issuance) build(|config: &GenesisConfig<T, I>| {
			config.balances.iter().fold(Zero::zero(), |acc: T::Balance, &(_, n)| acc + n)
		}): T::Balance;

		/// The balance of an account.
		///
		/// NOTE: THIS MAY NEVER BE IN EXISTENCE AND YET HAVE A `total().is_zero()`. If the total
		/// is ever zero, then the entry *MUST* be removed.
		///
		/// NOTE: This is only used in the case that this module is used to store balances.
		pub Account: map hasher(blake2_128_concat) T::AccountId => AccountData<T::Balance>;

		/// Any liquidity locks on some account balances.
		/// NOTE: Should only be accessed when setting, changing and freeing a lock.
		pub Locks get(fn locks): map hasher(blake2_128_concat) T::AccountId => Vec<BalanceLock<T::Balance>>;

		/// Storage version of the pallet.
		///
		/// This is set to v2.0.0 for new networks.
		StorageVersion build(|_: &GenesisConfig<T, I>| Releases::V2_0_0): Releases;
	}
	add_extra_genesis {
		config(balances): Vec<(T::AccountId, T::Balance)>;
		// ^^ begin, length, amount liquid at genesis
		build(|config: &GenesisConfig<T, I>| {
			assert!(
				<T as Trait<I>>::ExistentialDeposit::get() > Zero::zero(),
				"The existential deposit should be greater than zero."
			);
			for (_, balance) in &config.balances {
				assert!(
					*balance >= <T as Trait<I>>::ExistentialDeposit::get(),
					"the balance of any account should always be more than existential deposit.",
				)
			}
			for &(ref who, free) in config.balances.iter() {
				T::AccountStore::insert(who, AccountData { free, .. Default::default() });
			}
		});
	}
}

decl_module! {
	pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
		type Error = Error<T, I>;

		/// The minimum amount required to keep an account open.
		const ExistentialDeposit: T::Balance = T::ExistentialDeposit::get();

		fn deposit_event() = default;

		/// Transfer some liquid free balance to another account.
		///
		/// `transfer` will set the `FreeBalance` of the sender and receiver.
		/// It will decrease the total issuance of the system by the `TransferFee`.
		/// If the sender's account is below the existential deposit as a result
		/// of the transfer, the account will be reaped.
		///
		/// The dispatch origin for this call must be `Signed` by the transactor.
		///
		/// # <weight>
		/// - Dependent on arguments but not critical, given proper implementations for
		///   input config types. See related functions below.
		/// - It contains a limited number of reads and writes internally and no complex computation.
		///
		/// Related functions:
		///
		///   - `ensure_can_withdraw` is always called internally but has a bounded complexity.
		///   - Transferring balances to accounts that did not exist before will cause
		///      `T::OnNewAccount::on_new_account` to be called.
		///   - Removing enough funds from an account will trigger `T::DustRemoval::on_unbalanced`.
		///   - `transfer_keep_alive` works the same way as `transfer`, but has an additional
		///     check that the transfer will not kill the origin account.
		///
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(1_000_000)]
		pub fn transfer(
			origin,
			dest: <T::Lookup as StaticLookup>::Source,
			#[compact] value: T::Balance
		) {
			let transactor = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			//<Self as Currency<_>>::transfer(&transactor, &dest, value, ExistenceRequirement::AllowDeath)?;
            Err("transfers temporary locked")?
		}

		/// Set the balances of a given account.
		///
		/// This will alter `FreeBalance` and `ReservedBalance` in storage. it will
		/// also decrease the total issuance of the system (`TotalIssuance`).
		/// If the new free or reserved balance is below the existential deposit,
		/// it will reset the account nonce (`frame_system::AccountNonce`).
		///
		/// The dispatch origin for this call is `root`.
		///
		/// # <weight>
		/// - Independent of the arguments.
		/// - Contains a limited number of reads and writes.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedOperational(50_000)]
		fn set_balance(
			origin,
			who: <T::Lookup as StaticLookup>::Source,
			#[compact] new_free: T::Balance,
			#[compact] new_reserved: T::Balance
		) {
			ensure_root(origin)?;
			let who = T::Lookup::lookup(who)?;
			let existential_deposit = T::ExistentialDeposit::get();

			let wipeout = new_free + new_reserved < existential_deposit;
			let new_free = if wipeout { Zero::zero() } else { new_free };
			let new_reserved = if wipeout { Zero::zero() } else { new_reserved };

			let (free, reserved) = Self::mutate_account(&who, |account| {
				if new_free > account.free {
					mem::drop(PositiveImbalance::<T, I>::new(new_free - account.free));
				} else if new_free < account.free {
					mem::drop(NegativeImbalance::<T, I>::new(account.free - new_free));
				}

				if new_reserved > account.reserved {
					mem::drop(PositiveImbalance::<T, I>::new(new_reserved - account.reserved));
				} else if new_reserved < account.reserved {
					mem::drop(NegativeImbalance::<T, I>::new(account.reserved - new_reserved));
				}

				account.free = new_free;
				account.reserved = new_reserved;

				(account.free, account.reserved)
			});
			Self::deposit_event(RawEvent::BalanceSet(who, free, reserved));
		}

		/// Exactly as `transfer`, except the origin must be root and the source account may be
		/// specified.
		#[weight = SimpleDispatchInfo::FixedNormal(1_000_000)]
		pub fn force_transfer(
			origin,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			#[compact] value: T::Balance
		) {
			ensure_root(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Currency<_>>::transfer(&source, &dest, value, ExistenceRequirement::AllowDeath)?;
		}

		/// Same as the [`transfer`] call, but with a check that the transfer will not kill the
		/// origin account.
		///
		/// 99% of the time you want [`transfer`] instead.
		///
		/// [`transfer`]: struct.Module.html#method.transfer
		#[weight = SimpleDispatchInfo::FixedNormal(1_000_000)]
		pub fn transfer_keep_alive(
			origin,
			dest: <T::Lookup as StaticLookup>::Source,
			#[compact] value: T::Balance
		) {
			let transactor = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			//<Self as Currency<_>>::transfer(&transactor, &dest, value, KeepAlive)?;
            Err("transfers temporary locked")?
		}
	}
}

impl<T: Trait<I>, I: Instance> Module<T, I> {
	// PRIVATE MUTABLES

	/// Get the free balance of an account.
	pub fn free_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
		Self::account(who.borrow()).free
	}

	/// Get the balance of an account that can be used for transfers, reservations, or any other
	/// non-locking, non-transaction-fee activity. Will be at most `free_balance`.
	pub fn usable_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
		Self::account(who.borrow()).usable(Reasons::Misc)
	}

	/// Get the balance of an account that can be used for paying transaction fees (not tipping,
	/// or any other kind of fees, though). Will be at most `free_balance`.
	pub fn usable_balance_for_fees(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
		Self::account(who.borrow()).usable(Reasons::Fee)
	}

	/// Get the reserved balance of an account.
	pub fn reserved_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
		Self::account(who.borrow()).reserved
	}

	/// Get both the free and reserved balances of an account.
	fn account(who: &T::AccountId) -> AccountData<T::Balance> {
		T::AccountStore::get(&who)
	}

	/// Places the `free` and `reserved` parts of `new` into `account`. Also does any steps needed
	/// after mutating an account. This includes DustRemoval unbalancing, in the case than the `new`
	/// account's total balance is non-zero but below ED.
	///
	/// Returns the final free balance, iff the account was previously of total balance zero, known
	/// as its "endowment".
	fn post_mutation(
		who: &T::AccountId,
		new: AccountData<T::Balance>,
	) -> Option<AccountData<T::Balance>> {
		let total = new.total();
		if total < T::ExistentialDeposit::get() {
			if !total.is_zero() {
				T::DustRemoval::on_unbalanced(NegativeImbalance::new(total));
				Self::deposit_event(RawEvent::DustLost(who.clone(), total));
			}
			None
		} else {
			Some(new)
		}
	}

	/// Mutate an account to some new value, or delete it entirely with `None`. Will enforce
	/// `ExistentialDeposit` law, annulling the account as needed.
	///
	/// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
	/// when it is known that the account already exists.
	///
	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	fn mutate_account<R>(
		who: &T::AccountId,
		f: impl FnOnce(&mut AccountData<T::Balance>) -> R
	) -> R {
		Self::try_mutate_account(who, |a| -> Result<R, Infallible> { Ok(f(a)) })
			.expect("Error is infallible; qed")
	}

	/// Mutate an account to some new value, or delete it entirely with `None`. Will enforce
	/// `ExistentialDeposit` law, annulling the account as needed. This will do nothing if the
	/// result of `f` is an `Err`.
	///
	/// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
	/// when it is known that the account already exists.
	///
	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	fn try_mutate_account<R, E>(
		who: &T::AccountId,
		f: impl FnOnce(&mut AccountData<T::Balance>) -> Result<R, E>
	) -> Result<R, E> {
		T::AccountStore::try_mutate_exists(who, |maybe_account| {
			let mut account = maybe_account.take().unwrap_or_default();
			let was_zero = account.total().is_zero();
			f(&mut account).map(move |result| {
				let maybe_endowed = if was_zero { Some(account.free) } else { None };
				*maybe_account = Self::post_mutation(who, account);
				(maybe_endowed, result)
			})
		}).map(|(maybe_endowed, result)| {
			if let Some(endowed) = maybe_endowed {
				Self::deposit_event(RawEvent::Endowed(who.clone(), endowed));
			}
			result
		})
	}

	/// Update the account entry for `who`, given the locks.
	fn update_locks(who: &T::AccountId, locks: &[BalanceLock<T::Balance>]) {
		Self::mutate_account(who, |b| {
			b.misc_frozen = Zero::zero();
			b.fee_frozen = Zero::zero();
			for l in locks.iter() {
				if l.reasons == Reasons::All || l.reasons == Reasons::Misc {
					b.misc_frozen = b.misc_frozen.max(l.amount);
				}
				if l.reasons == Reasons::All || l.reasons == Reasons::Fee {
					b.fee_frozen = b.fee_frozen.max(l.amount);
				}
			}
		});

		let existed = Locks::<T, I>::contains_key(who);
		if locks.is_empty() {
			Locks::<T, I>::remove(who);
			if existed {
				// TODO: use Locks::<T, I>::hashed_key
				// https://github.com/paritytech/substrate/issues/4969
				system::Module::<T>::dec_ref(who);
			}
		} else {
			Locks::<T, I>::insert(who, locks);
			if !existed {
				system::Module::<T>::inc_ref(who);
			}
		}
	}
}

// wrapping these imbalances in a private module is necessary to ensure absolute privacy
// of the inner member.
mod imbalances {
	use super::{
		result, Subtrait, DefaultInstance, Imbalance, Trait, Zero, Instance, Saturating,
		StorageValue, TryDrop,
	};
	use sp_std::mem;

	/// Opaque, move-only struct with private fields that serves as a token denoting that
	/// funds have been created without any equal and opposite accounting.
	#[must_use]
	pub struct PositiveImbalance<T: Subtrait<I>, I: Instance=DefaultInstance>(T::Balance);

	impl<T: Subtrait<I>, I: Instance> PositiveImbalance<T, I> {
		/// Create a new positive imbalance from a balance.
		pub fn new(amount: T::Balance) -> Self {
			PositiveImbalance(amount)
		}
	}

	/// Opaque, move-only struct with private fields that serves as a token denoting that
	/// funds have been destroyed without any equal and opposite accounting.
	#[must_use]
	pub struct NegativeImbalance<T: Subtrait<I>, I: Instance=DefaultInstance>(T::Balance);

	impl<T: Subtrait<I>, I: Instance> NegativeImbalance<T, I> {
		/// Create a new negative imbalance from a balance.
		pub fn new(amount: T::Balance) -> Self {
			NegativeImbalance(amount)
		}
	}

	impl<T: Trait<I>, I: Instance> TryDrop for PositiveImbalance<T, I> {
		fn try_drop(self) -> result::Result<(), Self> {
			self.drop_zero()
		}
	}

	impl<T: Trait<I>, I: Instance> Imbalance<T::Balance> for PositiveImbalance<T, I> {
		type Opposite = NegativeImbalance<T, I>;

		fn zero() -> Self {
			Self(Zero::zero())
		}
		fn drop_zero(self) -> result::Result<(), Self> {
			if self.0.is_zero() {
				Ok(())
			} else {
				Err(self)
			}
		}
		fn split(self, amount: T::Balance) -> (Self, Self) {
			let first = self.0.min(amount);
			let second = self.0 - first;

			mem::forget(self);
			(Self(first), Self(second))
		}
		fn merge(mut self, other: Self) -> Self {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);

			self
		}
		fn subsume(&mut self, other: Self) {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);
		}
		fn offset(self, other: Self::Opposite) -> result::Result<Self, Self::Opposite> {
			let (a, b) = (self.0, other.0);
			mem::forget((self, other));

			if a >= b {
				Ok(Self(a - b))
			} else {
				Err(NegativeImbalance::new(b - a))
			}
		}
		fn peek(&self) -> T::Balance {
			self.0.clone()
		}
	}

	impl<T: Trait<I>, I: Instance> TryDrop for NegativeImbalance<T, I> {
		fn try_drop(self) -> result::Result<(), Self> {
			self.drop_zero()
		}
	}

	impl<T: Trait<I>, I: Instance> Imbalance<T::Balance> for NegativeImbalance<T, I> {
		type Opposite = PositiveImbalance<T, I>;

		fn zero() -> Self {
			Self(Zero::zero())
		}
		fn drop_zero(self) -> result::Result<(), Self> {
			if self.0.is_zero() {
				Ok(())
			} else {
				Err(self)
			}
		}
		fn split(self, amount: T::Balance) -> (Self, Self) {
			let first = self.0.min(amount);
			let second = self.0 - first;

			mem::forget(self);
			(Self(first), Self(second))
		}
		fn merge(mut self, other: Self) -> Self {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);

			self
		}
		fn subsume(&mut self, other: Self) {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);
		}
		fn offset(self, other: Self::Opposite) -> result::Result<Self, Self::Opposite> {
			let (a, b) = (self.0, other.0);
			mem::forget((self, other));

			if a >= b {
				Ok(Self(a - b))
			} else {
				Err(PositiveImbalance::new(b - a))
			}
		}
		fn peek(&self) -> T::Balance {
			self.0.clone()
		}
	}

	impl<T: Subtrait<I>, I: Instance> Drop for PositiveImbalance<T, I> {
		/// Basic drop handler will just square up the total issuance.
		fn drop(&mut self) {
			<super::TotalIssuance<super::ElevatedTrait<T, I>, I>>::mutate(
				|v| *v = v.saturating_add(self.0)
			);
		}
	}

	impl<T: Subtrait<I>, I: Instance> Drop for NegativeImbalance<T, I> {
		/// Basic drop handler will just square up the total issuance.
		fn drop(&mut self) {
			<super::TotalIssuance<super::ElevatedTrait<T, I>, I>>::mutate(
				|v| *v = v.saturating_sub(self.0)
			);
		}
	}
}

// TODO: #2052
// Somewhat ugly hack in order to gain access to module's `increase_total_issuance_by`
// using only the Subtrait (which defines only the types that are not dependent
// on Positive/NegativeImbalance). Subtrait must be used otherwise we end up with a
// circular dependency with Trait having some types be dependent on PositiveImbalance<Trait>
// and PositiveImbalance itself depending back on Trait for its Drop impl (and thus
// its type declaration).
// This works as long as `increase_total_issuance_by` doesn't use the Imbalance
// types (basically for charging fees).
// This should eventually be refactored so that the type item that
// depends on the Imbalance type (DustRemoval) is placed in its own pallet.
struct ElevatedTrait<T: Subtrait<I>, I: Instance>(T, I);
impl<T: Subtrait<I>, I: Instance> Clone for ElevatedTrait<T, I> {
	fn clone(&self) -> Self { unimplemented!() }
}
impl<T: Subtrait<I>, I: Instance> PartialEq for ElevatedTrait<T, I> {
	fn eq(&self, _: &Self) -> bool { unimplemented!() }
}
impl<T: Subtrait<I>, I: Instance> Eq for ElevatedTrait<T, I> {}
impl<T: Subtrait<I>, I: Instance> frame_system::Trait for ElevatedTrait<T, I> {
	type Origin = T::Origin;
	type Call = T::Call;
	type Index = T::Index;
	type BlockNumber = T::BlockNumber;
	type Hash = T::Hash;
	type Hashing = T::Hashing;
	type AccountId = T::AccountId;
	type Lookup = T::Lookup;
	type Header = T::Header;
	type Event = ();
	type BlockHashCount = T::BlockHashCount;
	type MaximumBlockWeight = T::MaximumBlockWeight;
	type MaximumBlockLength = T::MaximumBlockLength;
	type AvailableBlockRatio = T::AvailableBlockRatio;
	type Version = T::Version;
	type ModuleToIndex = T::ModuleToIndex;
	type OnNewAccount = T::OnNewAccount;
	type OnKilledAccount = T::OnKilledAccount;
	type AccountData = T::AccountData;
}
impl<T: Subtrait<I>, I: Instance> Trait<I> for ElevatedTrait<T, I> {
	type Balance = T::Balance;
	type Event = ();
	type DustRemoval = ();
	type ExistentialDeposit = T::ExistentialDeposit;
	type AccountStore = T::AccountStore;
}

impl<T: Trait<I>, I: Instance> Currency<T::AccountId> for Module<T, I> where
	T::Balance: MaybeSerializeDeserialize + Debug
{
	type Balance = T::Balance;
	type PositiveImbalance = PositiveImbalance<T, I>;
	type NegativeImbalance = NegativeImbalance<T, I>;

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		Self::account(who).total()
	}

	// Check if `value` amount of free balance can be slashed from `who`.
	fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
		if value.is_zero() { return true }
		Self::free_balance(who) >= value
	}

	fn total_issuance() -> Self::Balance {
		<TotalIssuance<T, I>>::get()
	}

	fn minimum_balance() -> Self::Balance {
		T::ExistentialDeposit::get()
	}

	// Burn funds from the total issuance, returning a positive imbalance for the amount burned.
	// Is a no-op if amount to be burned is zero.
	fn burn(mut amount: Self::Balance) -> Self::PositiveImbalance {
		if amount.is_zero() { return PositiveImbalance::zero() }
		<TotalIssuance<T, I>>::mutate(|issued| {
			*issued = issued.checked_sub(&amount).unwrap_or_else(|| {
				amount = *issued;
				Zero::zero()
			});
		});
		PositiveImbalance::new(amount)
	}

	// Create new funds into the total issuance, returning a negative imbalance
	// for the amount issued.
	// Is a no-op if amount to be issued it zero.
	fn issue(mut amount: Self::Balance) -> Self::NegativeImbalance {
		if amount.is_zero() { return NegativeImbalance::zero() }
		<TotalIssuance<T, I>>::mutate(|issued|
			*issued = issued.checked_add(&amount).unwrap_or_else(|| {
				amount = Self::Balance::max_value() - *issued;
				Self::Balance::max_value()
			})
		);
		NegativeImbalance::new(amount)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		Self::account(who).free
	}

	// Ensure that an account can withdraw from their free balance given any existing withdrawal
	// restrictions like locks and vesting balance.
	// Is a no-op if amount to be withdrawn is zero.
	//
	// # <weight>
	// Despite iterating over a list of locks, they are limited by the number of
	// lock IDs, which means the number of runtime modules that intend to use and create locks.
	// # </weight>
	fn ensure_can_withdraw(
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
		new_balance: T::Balance,
	) -> DispatchResult {
		if amount.is_zero() { return Ok(()) }
		let min_balance = Self::account(who).frozen(reasons.into());
		ensure!(new_balance >= min_balance, Error::<T, I>::LiquidityRestrictions);
		Ok(())
	}

	// Transfer some free balance from `transactor` to `dest`, respecting existence requirements.
	// Is a no-op if value to be transferred is zero or the `transactor` is the same as `dest`.
	fn transfer(
		transactor: &T::AccountId,
		dest: &T::AccountId,
		value: Self::Balance,
		existence_requirement: ExistenceRequirement,
	) -> DispatchResult {
		if value.is_zero() || transactor == dest { return Ok(()) }

		Self::try_mutate_account(dest, |to_account| -> DispatchResult {
			Self::try_mutate_account(transactor, |from_account| -> DispatchResult {
				from_account.free = from_account.free.checked_sub(&value)
					.ok_or(Error::<T, I>::InsufficientBalance)?;

				// NOTE: total stake being stored in the same type means that this could never overflow
				// but better to be safe than sorry.
				to_account.free = to_account.free.checked_add(&value).ok_or(Error::<T, I>::Overflow)?;

				let ed = T::ExistentialDeposit::get();
				ensure!(to_account.total() >= ed, Error::<T, I>::ExistentialDeposit);

				Self::ensure_can_withdraw(
					transactor,
					value,
					WithdrawReason::Transfer.into(),
					from_account.free,
				)?;

				let allow_death = existence_requirement == ExistenceRequirement::AllowDeath;
				let allow_death = allow_death && system::Module::<T>::allow_death(transactor);
				ensure!(allow_death || from_account.free >= ed, Error::<T, I>::KeepAlive);

				Ok(())
			})
		})?;

		// Emit transfer event.
		Self::deposit_event(RawEvent::Transfer(transactor.clone(), dest.clone(), value));

		Ok(())
	}

	/// Slash a target account `who`, returning the negative imbalance created and any left over
	/// amount that could not be slashed.
	///
	/// Is a no-op if `value` to be slashed is zero.
	///
	/// NOTE: `slash()` prefers free balance, but assumes that reserve balance can be drawn
	/// from in extreme circumstances. `can_slash()` should be used prior to `slash()` to avoid having
	/// to draw from reserved funds, however we err on the side of punishment if things are inconsistent
	/// or `can_slash` wasn't used appropriately.
	fn slash(
		who: &T::AccountId,
		value: Self::Balance
	) -> (Self::NegativeImbalance, Self::Balance) {
		if value.is_zero() { return (NegativeImbalance::zero(), Zero::zero()) }

		Self::mutate_account(who, |account| {
			let free_slash = cmp::min(account.free, value);
			account.free -= free_slash;

			let remaining_slash = value - free_slash;
			if !remaining_slash.is_zero() {
				let reserved_slash = cmp::min(account.reserved, remaining_slash);
				account.reserved -= reserved_slash;
				(NegativeImbalance::new(free_slash + reserved_slash), remaining_slash - reserved_slash)
			} else {
				(NegativeImbalance::new(value), Zero::zero())
			}
		})
	}

	/// Deposit some `value` into the free balance of an existing target account `who`.
	///
	/// Is a no-op if the `value` to be deposited is zero.
	fn deposit_into_existing(
		who: &T::AccountId,
		value: Self::Balance
	) -> Result<Self::PositiveImbalance, DispatchError> {
		if value.is_zero() { return Ok(PositiveImbalance::zero()) }

		Self::try_mutate_account(who, |account| -> Result<Self::PositiveImbalance, DispatchError> {
			ensure!(!account.total().is_zero(), Error::<T, I>::DeadAccount);
			account.free = account.free.checked_add(&value).ok_or(Error::<T, I>::Overflow)?;
			Ok(PositiveImbalance::new(value))
		})
	}

	/// Deposit some `value` into the free balance of `who`, possibly creating a new account.
	///
	/// This function is a no-op if:
	/// - the `value` to be deposited is zero; or
	/// - if the `value` to be deposited is less than the ED and the account does not yet exist; or
	/// - `value` is so large it would cause the balance of `who` to overflow.
	fn deposit_creating(
		who: &T::AccountId,
		value: Self::Balance,
	) -> Self::PositiveImbalance {
		if value.is_zero() { return Self::PositiveImbalance::zero() }

		Self::try_mutate_account(who, |account| -> Result<Self::PositiveImbalance, Self::PositiveImbalance> {
			// bail if not yet created and this operation wouldn't be enough to create it.
			let ed = T::ExistentialDeposit::get();
			ensure!(value >= ed || !account.total().is_zero(), Self::PositiveImbalance::zero());

			// defensive only: overflow should never happen, however in case it does, then this
			// operation is a no-op.
			account.free = account.free.checked_add(&value).ok_or(Self::PositiveImbalance::zero())?;

			Ok(PositiveImbalance::new(value))
		}).unwrap_or_else(|x| x)
	}

	/// Withdraw some free balance from an account, respecting existence requirements.
	///
	/// Is a no-op if value to be withdrawn is zero.
	fn withdraw(
		who: &T::AccountId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> result::Result<Self::NegativeImbalance, DispatchError> {
		if value.is_zero() { return Ok(NegativeImbalance::zero()); }

		Self::try_mutate_account(who, |account|
			-> Result<Self::NegativeImbalance, DispatchError>
		{
			let new_free_account = account.free.checked_sub(&value)
				.ok_or(Error::<T, I>::InsufficientBalance)?;

			// bail if we need to keep the account alive and this would kill it.
			let ed = T::ExistentialDeposit::get();
			let would_be_dead = new_free_account + account.reserved < ed;
			let would_kill = would_be_dead && account.free + account.reserved >= ed;
			ensure!(liveness == AllowDeath || !would_kill, Error::<T, I>::KeepAlive);

			Self::ensure_can_withdraw(who, value, reasons, new_free_account)?;

			account.free = new_free_account;

			Ok(NegativeImbalance::new(value))
		})
	}

	/// Force the new free balance of a target account `who` to some new value `balance`.
	fn make_free_balance_be(who: &T::AccountId, value: Self::Balance)
		-> SignedImbalance<Self::Balance, Self::PositiveImbalance>
	{
		Self::try_mutate_account(who, |account|
			-> Result<SignedImbalance<Self::Balance, Self::PositiveImbalance>, ()>
		{
			let ed = T::ExistentialDeposit::get();
			// If we're attempting to set an existing account to less than ED, then
			// bypass the entire operation. It's a no-op if you follow it through, but
			// since this is an instance where we might account for a negative imbalance
			// (in the dust cleaner of set_account) before we account for its actual
			// equal and opposite cause (returned as an Imbalance), then in the
			// instance that there's no other accounts on the system at all, we might
			// underflow the issuance and our arithmetic will be off.
			ensure!(value + account.reserved >= ed || !account.total().is_zero(), ());

			let imbalance = if account.free <= value {
				SignedImbalance::Positive(PositiveImbalance::new(value - account.free))
			} else {
				SignedImbalance::Negative(NegativeImbalance::new(account.free - value))
			};
			account.free = value;
			Ok(imbalance)
		}).unwrap_or(SignedImbalance::Positive(Self::PositiveImbalance::zero()))
	}
}

impl<T: Trait<I>, I: Instance> ReservableCurrency<T::AccountId> for Module<T, I>  where
	T::Balance: MaybeSerializeDeserialize + Debug
{
	/// Check if `who` can reserve `value` from their free balance.
	///
	/// Always `true` if value to be reserved is zero.
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		if value.is_zero() { return true }
		Self::account(who).free
			.checked_sub(&value)
			.map_or(false, |new_balance|
				Self::ensure_can_withdraw(who, value, WithdrawReason::Reserve.into(), new_balance).is_ok()
			)
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		Self::account(who).reserved
	}

	/// Move `value` from the free balance from `who` to their reserved balance.
	///
	/// Is a no-op if value to be reserved is zero.
	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		if value.is_zero() { return Ok(()) }

		Self::try_mutate_account(who, |account| -> DispatchResult {
			account.free = account.free.checked_sub(&value).ok_or(Error::<T, I>::InsufficientBalance)?;
			account.reserved = account.reserved.checked_add(&value).ok_or(Error::<T, I>::Overflow)?;
			Self::ensure_can_withdraw(who, value, WithdrawReason::Reserve.into(), account.free)
		})
	}

	/// Unreserve some funds, returning any amount that was unable to be unreserved.
	///
	/// Is a no-op if the value to be unreserved is zero.
	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		if value.is_zero() { return Zero::zero() }

		Self::mutate_account(who, |account| {
			let actual = cmp::min(account.reserved, value);
			account.reserved -= actual;
			// defensive only: this can never fail since total issuance which is at least free+reserved
			// fits into the same data type.
			account.free = account.free.saturating_add(actual);
			value - actual
		})
	}

	/// Slash from reserved balance, returning the negative imbalance created,
	/// and any amount that was unable to be slashed.
	///
	/// Is a no-op if the value to be slashed is zero.
	fn slash_reserved(
		who: &T::AccountId,
		value: Self::Balance
	) -> (Self::NegativeImbalance, Self::Balance) {
		if value.is_zero() { return (NegativeImbalance::zero(), Zero::zero()) }

		Self::mutate_account(who, |account| {
			// underflow should never happen, but it if does, there's nothing to be done here.
			let actual = cmp::min(account.reserved, value);
			account.reserved -= actual;
			(NegativeImbalance::new(actual), value - actual)
		})
	}

	/// Move the reserved balance of one account into the balance of another, according to `status`.
	///
	/// Is a no-op if:
	/// - the value to be moved is zero; or
	/// - the `slashed` id equal to `beneficiary` and the `status` is `Reserved`.
	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: Status,
	) -> Result<Self::Balance, DispatchError> {
		if value.is_zero() { return Ok(Zero::zero()) }

		if slashed == beneficiary {
			return match status {
				Status::Free => Ok(Self::unreserve(slashed, value)),
				Status::Reserved => Ok(value.saturating_sub(Self::reserved_balance(slashed))),
			};
		}

		Self::try_mutate_account(beneficiary, |to_account| -> Result<Self::Balance, DispatchError> {
			ensure!(!to_account.total().is_zero(), Error::<T, I>::DeadAccount);
			Self::try_mutate_account(slashed, |from_account| -> Result<Self::Balance, DispatchError> {
				let actual = cmp::min(from_account.reserved, value);
				match status {
					Status::Free => to_account.free = to_account.free.checked_add(&actual).ok_or(Error::<T, I>::Overflow)?,
					Status::Reserved => to_account.reserved = to_account.reserved.checked_add(&actual).ok_or(Error::<T, I>::Overflow)?,
				}
				from_account.reserved -= actual;
				Ok(value - actual)
			})
		})
	}
}

/// Implement `OnKilledAccount` to remove the local account, if using local account storage.
///
/// NOTE: You probably won't need to use this! This only needs to be "wired in" to System module
/// if you're using the local balance storage. **If you're using the composite system account
/// storage (which is the default in most examples and tests) then there's no need.**
impl<T: Trait<I>, I: Instance> OnKilledAccount<T::AccountId> for Module<T, I> {
	fn on_killed_account(who: &T::AccountId) {
		Account::<T, I>::remove(who);
	}
}

impl<T: Trait<I>, I: Instance> LockableCurrency<T::AccountId> for Module<T, I>
where
	T::Balance: MaybeSerializeDeserialize + Debug
{
	type Moment = T::BlockNumber;

	// Set a lock on the balance of `who`.
	// Is a no-op if lock amount is zero or `reasons` `is_none()`.
	fn set_lock(
		id: LockIdentifier,
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
	) {
		if amount.is_zero() || reasons.is_none() { return }
		let mut new_lock = Some(BalanceLock { id, amount, reasons: reasons.into() });
		let mut locks = Self::locks(who).into_iter().filter_map(|l|
			if l.id == id {
				new_lock.take()
			} else {
				Some(l)
			}).collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		Self::update_locks(who, &locks[..]);
	}

	// Extend a lock on the balance of `who`.
	// Is a no-op if lock amount is zero or `reasons` `is_none()`.
	fn extend_lock(
		id: LockIdentifier,
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
	) {
		if amount.is_zero() || reasons.is_none() { return }
		let mut new_lock = Some(BalanceLock { id, amount, reasons: reasons.into() });
		let mut locks = Self::locks(who).into_iter().filter_map(|l|
			if l.id == id {
				new_lock.take().map(|nl| {
					BalanceLock {
						id: l.id,
						amount: l.amount.max(nl.amount),
						reasons: l.reasons | nl.reasons,
					}
				})
			} else {
				Some(l)
			}).collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		Self::update_locks(who, &locks[..]);
	}

	fn remove_lock(
		id: LockIdentifier,
		who: &T::AccountId,
	) {
		let mut locks = Self::locks(who);
		locks.retain(|l| l.id != id);
		Self::update_locks(who, &locks[..]);
	}
}

impl<T: Trait<I>, I: Instance> IsDeadAccount<T::AccountId> for Module<T, I> where
	T::Balance: MaybeSerializeDeserialize + Debug
{
	fn is_dead_account(who: &T::AccountId) -> bool {
		// this should always be exactly equivalent to `Self::account(who).total().is_zero()`
		!T::AccountStore::is_explicit(who)
	}
}
