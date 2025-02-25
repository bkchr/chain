/*
 * This file is part of the Nodle Chain distributed at https://github.com/NodleCode/chain
 * Copyright (C) 2020-2022  Nodle International
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

#![cfg_attr(not(feature = "std"), no_std)]

//! A module that is called by the `collective` and is in charge of holding
//! the company funds.
mod benchmarking;

#[cfg(test)]
mod tests;

use frame_support::{
	dispatch::GetDispatchInfo,
	traits::{Currency, ExistenceRequirement, Get, Imbalance, OnUnbalanced},
	PalletId,
};
use sp_runtime::traits::{AccountIdConversion, Dispatchable};
use sp_std::prelude::Box;
use support::WithAccountId;

#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;

pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

type BalanceOf<T, I> = <<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T, I> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeEvent: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type ExternalOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type Currency: Currency<Self::AccountId>;
		type RuntimeCall: Parameter + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin> + GetDispatchInfo;
		type PalletId: Get<PalletId>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Spend `amount` funds from the reserve account to `to`.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::spend())]
		pub fn spend(origin: OriginFor<T>, to: T::AccountId, amount: BalanceOf<T, I>) -> DispatchResultWithPostInfo {
			T::ExternalOrigin::try_origin(origin).map(|_| ()).or_else(ensure_root)?;

			let _ = T::Currency::transfer(&Self::account_id(), &to, amount, ExistenceRequirement::KeepAlive);

			Self::deposit_event(Event::SpentFunds(to, amount));

			Ok(().into())
		}

		/// Deposit `amount` tokens in the treasure account
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::tip())]
		pub fn tip(origin: OriginFor<T>, amount: BalanceOf<T, I>) -> DispatchResultWithPostInfo {
			let tipper = ensure_signed(origin)?;

			let _ = T::Currency::transfer(&tipper, &Self::account_id(), amount, ExistenceRequirement::AllowDeath);

			Self::deposit_event(Event::TipReceived(tipper, amount));

			Ok(().into())
		}

		#[allow(clippy::boxed_local)]
		/// Dispatch a call as coming from the reserve account
		#[pallet::call_index(2)]
		#[pallet::weight({
            let dispatch_info = call.get_dispatch_info();
            (
                dispatch_info.weight.saturating_add(Weight::from_parts(10_000, 0)),
                dispatch_info.class,
            )
        })]
		pub fn apply_as(origin: OriginFor<T>, call: Box<<T as Config<I>>::RuntimeCall>) -> DispatchResultWithPostInfo {
			T::ExternalOrigin::try_origin(origin).map(|_| ()).or_else(ensure_root)?;

			let res = call.dispatch(frame_system::RawOrigin::Signed(Self::account_id()).into());

			Self::deposit_event(Event::ReserveOp(res.map(|_| ()).map_err(|e| e.error)));

			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Some amount was deposited (e.g. for transaction fees).
		Deposit(BalanceOf<T, I>),
		/// Some funds were spent from the reserve.
		SpentFunds(T::AccountId, BalanceOf<T, I>),
		/// Someone tipped the company reserve
		TipReceived(T::AccountId, BalanceOf<T, I>),
		/// We executed a call coming from the company reserve account
		ReserveOp(DispatchResult),
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		pub phantom: sp_std::marker::PhantomData<(T, I)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self {
				phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
		fn build(&self) {
			let our_account = &<Pallet<T, I>>::account_id();

			if T::Currency::free_balance(our_account) < T::Currency::minimum_balance() {
				let _ = T::Currency::make_free_balance_be(our_account, T::Currency::minimum_balance());
			}
		}
	}
}

impl<T: Config<I>, I: 'static> WithAccountId<T::AccountId> for Pallet<T, I> {
	fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}

impl<T: Config<I>, I: 'static> OnUnbalanced<NegativeImbalanceOf<T, I>> for Pallet<T, I> {
	fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
		let numeric_amount = amount.peek();

		// Must resolve into existing but better to be safe.
		T::Currency::resolve_creating(&Self::account_id(), amount);

		Self::deposit_event(Event::Deposit(numeric_amount));
	}
}
