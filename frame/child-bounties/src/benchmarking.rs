// This file is part of Substrate.

// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Child-bounties pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

use crate::Pallet as ChildBounties;
use pallet_bounties::Pallet as Bounties;
use pallet_treasury::Pallet as Treasury;

const SEED: u32 = 0;
const MAX_BYTES: u32 = 16384;

#[derive(Clone)]
struct BenchmarkChildBounty<T: Config> {
	/// Bounty ID.
	bounty_id: BountyIndex,
	/// ChildBounty ID.
	child_bounty_id: BountyIndex,
	/// The account proposing it.
	caller: T::AccountId,
	/// The master curator account.
	curator: T::AccountId,
	/// The child-bounty curator account.
	child_curator: T::AccountId,
	/// The (total) amount that should be paid if the bounty is rewarded.
	value: BalanceOf<T>,
	/// The curator fee. included in value.
	fee: BalanceOf<T>,
	/// The (total) amount that should be paid if the child-bounty is rewarded.
	child_bounty_value: BalanceOf<T>,
	/// The child-bounty curator fee. included in value.
	child_bounty_fee: BalanceOf<T>,
	/// Bounty description.
	reason: Vec<u8>,
}

fn setup_bounty<T: Config>(
	u: u32,
	d: u32,
) -> (T::AccountId, T::AccountId, BalanceOf<T>, BalanceOf<T>, Vec<u8>) {
	let caller = account("caller", u, SEED);
	let value: BalanceOf<T> = T::BountyValueMinimum::get().saturating_mul(100u32.into());
	let fee = value / 2u32.into();
	let deposit = T::BountyDepositBase::get() + T::DataDepositPerByte::get() * MAX_BYTES.into();
	let _ = T::Currency::make_free_balance_be(&caller, deposit);
	let curator = account("curator", u, SEED);
	let _ = T::Currency::make_free_balance_be(&curator, fee / 2u32.into());
	let reason = vec![0; d as usize];
	(caller, curator, fee, value, reason)
}

fn setup_child_bounty<T: Config>(u: u32, d: u32) -> BenchmarkChildBounty<T> {
	let (caller, curator, fee, value, reason) = setup_bounty::<T>(u, d);
	let child_curator = account("child-curator", u, SEED);
	let _ = T::Currency::make_free_balance_be(&child_curator, fee / 2u32.into());
	let child_bounty_value = (value - fee) / 4u32.into();
	let child_bounty_fee = child_bounty_value / 2u32.into();

	BenchmarkChildBounty::<T> {
		bounty_id: 0,
		child_bounty_id: 0,
		caller,
		curator,
		child_curator,
		value,
		fee,
		child_bounty_value,
		child_bounty_fee,
		reason,
	}
}

fn create_bounty<T: Config>(u: u32, d: u32) -> Result<BenchmarkChildBounty<T>, &'static str> {
	let mut bounty_setup = setup_child_bounty::<T>(u, d);
	let curator_lookup = T::Lookup::unlookup(bounty_setup.curator.clone());
	Bounties::<T>::propose_bounty(
		RawOrigin::Signed(bounty_setup.caller.clone()).into(),
		bounty_setup.value,
		bounty_setup.reason.clone(),
	)?;

	bounty_setup.bounty_id = Bounties::<T>::bounty_count() - 1;

	Bounties::<T>::approve_bounty(RawOrigin::Root.into(), bounty_setup.bounty_id)?;
	Treasury::<T>::on_initialize(T::BlockNumber::zero());
	Bounties::<T>::propose_curator(
		RawOrigin::Root.into(),
		bounty_setup.bounty_id,
		curator_lookup.clone(),
		bounty_setup.fee,
	)?;
	Bounties::<T>::accept_curator(
		RawOrigin::Signed(bounty_setup.curator.clone()).into(),
		bounty_setup.bounty_id,
	)?;

	Ok(bounty_setup)
}

fn create_child_bounty<T: Config>(u: u32, d: u32) -> Result<BenchmarkChildBounty<T>, &'static str> {
	let mut bounty_setup = create_bounty::<T>(u, d)?;
	let child_curator_lookup = T::Lookup::unlookup(bounty_setup.child_curator.clone());

	ChildBounties::<T>::add_child_bounty(
		RawOrigin::Signed(bounty_setup.curator.clone()).into(),
		bounty_setup.bounty_id,
		bounty_setup.child_bounty_value,
		bounty_setup.reason.clone(),
	)?;

	bounty_setup.child_bounty_id = ChildBountyCount::<T>::get() - 1;

	ChildBounties::<T>::propose_curator(
		RawOrigin::Signed(bounty_setup.curator.clone()).into(),
		bounty_setup.bounty_id,
		bounty_setup.child_bounty_id,
		child_curator_lookup.clone(),
		bounty_setup.child_bounty_fee,
	)?;

	ChildBounties::<T>::accept_curator(
		RawOrigin::Signed(bounty_setup.child_curator.clone()).into(),
		bounty_setup.bounty_id,
		bounty_setup.child_bounty_id,
	)?;

	Ok(bounty_setup)
}

fn setup_pot_account<T: Config>() {
	let pot_account = Bounties::<T>::account_id();
	let value = T::Currency::minimum_balance().saturating_mul(1_000_000_000u32.into());
	let _ = T::Currency::make_free_balance_be(&pot_account, value);
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	add_child_bounty {
		let d in 0 .. MAX_BYTES;
		setup_pot_account::<T>();
		let bounty_setup = create_bounty::<T>(0, d)?;
	}: _(RawOrigin::Signed(bounty_setup.curator), bounty_setup.bounty_id,
			bounty_setup.child_bounty_value, bounty_setup.reason.clone())
	verify {
		assert_last_event::<T>(Event::ChildBountyAdded(bounty_setup.bounty_id,
			bounty_setup.child_bounty_id).into())
	}

	propose_curator {
		setup_pot_account::<T>();
		let mut bounty_setup = create_bounty::<T>(0, MAX_BYTES)?;
		let child_curator_lookup = T::Lookup::unlookup(bounty_setup.child_curator.clone());

		ChildBounties::<T>::add_child_bounty(
			RawOrigin::Signed(bounty_setup.curator.clone()).into(),
			bounty_setup.bounty_id,
			bounty_setup.child_bounty_value,
			bounty_setup.reason.clone(),
		)?;
		bounty_setup.child_bounty_id = ChildBountyCount::<T>::get() - 1;

	}: _(RawOrigin::Signed(bounty_setup.curator), bounty_setup.bounty_id,
			bounty_setup.child_bounty_id, child_curator_lookup, bounty_setup.child_bounty_fee)

	accept_curator {
		setup_pot_account::<T>();
		let mut bounty_setup = create_bounty::<T>(0, MAX_BYTES)?;
		let child_curator_lookup = T::Lookup::unlookup(bounty_setup.child_curator.clone());

		ChildBounties::<T>::add_child_bounty(
			RawOrigin::Signed(bounty_setup.curator.clone()).into(),
			bounty_setup.bounty_id,
			bounty_setup.child_bounty_value,
			bounty_setup.reason.clone(),
		)?;
		bounty_setup.child_bounty_id = ChildBountyCount::<T>::get() - 1;

		ChildBounties::<T>::propose_curator(
			RawOrigin::Signed(bounty_setup.curator.clone()).into(),
			bounty_setup.bounty_id,
			bounty_setup.child_bounty_id,
			child_curator_lookup.clone(),
			bounty_setup.child_bounty_fee,
		)?;
	}: _(RawOrigin::Signed(bounty_setup.child_curator), bounty_setup.bounty_id,
			bounty_setup.child_bounty_id)

	// Worst case when curator is inactive and any sender un-assigns the curator.
	unassign_curator {
		setup_pot_account::<T>();
		let bounty_setup = create_child_bounty::<T>(0, MAX_BYTES)?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
		frame_system::Pallet::<T>::set_block_number(T::BountyUpdatePeriod::get() + 1u32.into());
		let caller = whitelisted_caller();
	}: _(RawOrigin::Signed(caller), bounty_setup.bounty_id,
			bounty_setup.child_bounty_id)

	award_child_bounty {
		setup_pot_account::<T>();
		let bounty_setup = create_child_bounty::<T>(0, MAX_BYTES)?;
		let beneficiary_account: T::AccountId = account("beneficiary", 0, SEED);
		let beneficiary = T::Lookup::unlookup(beneficiary_account.clone());
	}: _(RawOrigin::Signed(bounty_setup.child_curator), bounty_setup.bounty_id,
			bounty_setup.child_bounty_id, beneficiary)
	verify {
		assert_last_event::<T>(Event::ChildBountyAwarded(bounty_setup.bounty_id,
			bounty_setup.child_bounty_id, beneficiary_account).into())
	}

	claim_child_bounty {
		setup_pot_account::<T>();
		let bounty_setup = create_child_bounty::<T>(0, MAX_BYTES)?;
		let beneficiary_account: T::AccountId = account("beneficiary", 0, SEED);
		let beneficiary = T::Lookup::unlookup(beneficiary_account.clone());

		ChildBounties::<T>::award_child_bounty(
			RawOrigin::Signed(bounty_setup.child_curator.clone()).into(),
			bounty_setup.bounty_id,
			bounty_setup.child_bounty_id,
			beneficiary
		)?;

		let beneficiary_account: T::AccountId = account("beneficiary", 0, SEED);
		let beneficiary = T::Lookup::unlookup(beneficiary_account.clone());

		frame_system::Pallet::<T>::set_block_number(T::BountyDepositPayoutDelay::get());
		ensure!(T::Currency::free_balance(&beneficiary_account).is_zero(),
			"Beneficiary already has balance.");

	}: _(RawOrigin::Signed(bounty_setup.curator), bounty_setup.bounty_id,
			bounty_setup.child_bounty_id)
	verify {
		ensure!(!T::Currency::free_balance(&beneficiary_account).is_zero(),
			"Beneficiary didn't get paid.");
	}

	// Best case scenario.
	close_child_bounty_added {
		setup_pot_account::<T>();
		let mut bounty_setup = create_bounty::<T>(0, MAX_BYTES)?;

		ChildBounties::<T>::add_child_bounty(
			RawOrigin::Signed(bounty_setup.curator.clone()).into(),
			bounty_setup.bounty_id,
			bounty_setup.child_bounty_value,
			bounty_setup.reason.clone(),
		)?;
		bounty_setup.child_bounty_id = ChildBountyCount::<T>::get() - 1;

	}: close_child_bounty(RawOrigin::Root, bounty_setup.bounty_id,
		bounty_setup.child_bounty_id)
	verify {
		assert_last_event::<T>(Event::ChildBountyCanceled(bounty_setup.bounty_id,
			bounty_setup.child_bounty_id).into())
	}

	// Worst case scenario.
	close_child_bounty_active {
		setup_pot_account::<T>();
		let bounty_setup = create_child_bounty::<T>(0, MAX_BYTES)?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
	}: close_child_bounty(RawOrigin::Root, bounty_setup.bounty_id, bounty_setup.child_bounty_id)
	verify {
		assert_last_event::<T>(Event::ChildBountyCanceled(bounty_setup.bounty_id,
			bounty_setup.child_bounty_id).into())
	}

	impl_benchmark_test_suite!(ChildBounties, crate::tests::new_test_ext(), crate::tests::Test)
}