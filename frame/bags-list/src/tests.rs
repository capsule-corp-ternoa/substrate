use frame_support::assert_ok;

use super::*;
use frame_election_provider_support::SortedListProvider;
use list::Bag;
use mock::{ext_builder::*, test_utils::*, *};

mod extrinsics {
	use super::*;

	#[test]
	fn rebag_works() {
		ExtBuilder::default().add_ids(vec![(42, 20)]).build_and_execute(|| {
			// given
			assert_eq!(get_bags(), vec![(10, vec![1]), (20, vec![42]), (1000, vec![2, 3, 4])]);

			// increase vote weight and implicitly rebag to the level of non-existent bag
			NextVoteWeight::set(2000);
			assert_ok!(BagsList::rebag(Origin::signed(0), 42));
			assert_eq!(get_bags(), vec![(10, vec![1]), (1000, vec![2, 3, 4]), (2000, vec![42])]);

			// decrease weight within the range of the current bag
			NextVoteWeight::set(1001);
			assert_ok!(BagsList::rebag(Origin::signed(0), 42));
			// does not change bags
			assert_eq!(get_bags(), vec![(10, vec![1]), (1000, vec![2, 3, 4]), (2000, vec![42])]);

			// reduce weight to the level of a non-existent bag
			NextVoteWeight::set(30);
			assert_ok!(BagsList::rebag(Origin::signed(0), 42));
			// creates the bag and moves the voter into it
			assert_eq!(get_bags(), vec![(10, vec![1]), (30, vec![42]), (1000, vec![2, 3, 4])]);

			// increase weight to a pre-existing bag
			NextVoteWeight::set(500);
			assert_ok!(BagsList::rebag(Origin::signed(0), 42));
			// moves the voter to that bag
			assert_eq!(get_bags(), vec![(10, vec![1]), (1000, vec![2, 3, 4, 42])]);
		});
	}

	// Rebagging the tail of a bag results in the old bag having a new tail and an overall correct
	// state.
	#[test]
	fn rebag_tail_works() {
		ExtBuilder::default().build_and_execute(|| {
			// given
			assert_eq!(get_bags(), vec![(10, vec![1]), (1000, vec![2, 3, 4])]);

			// when
			NextVoteWeight::set(10);
			assert_ok!(BagsList::rebag(Origin::signed(0), 4));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 4]), (1000, vec![2, 3])]);
			assert_eq!(Bag::<Runtime>::get(1_000).unwrap(), Bag::new(Some(2), Some(3), 1_000));

			// when
			assert_ok!(BagsList::rebag(Origin::signed(0), 3));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 4, 3]), (1000, vec![2])]);

			assert_eq!(Bag::<Runtime>::get(10).unwrap(), Bag::new(Some(1), Some(3), 10));
			// TODO: This might be wrong, should be None.
			assert_eq!(Bag::<Runtime>::get(1000).unwrap(), Bag::new(Some(2), Some(2), 1000));
			assert_eq!(get_voter_list_as_ids(), vec![2u32, 1, 4, 3]);

			// when
			assert_ok!(BagsList::rebag(Origin::signed(0), 2));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 4, 3, 2])]);
		});
	}

	// Rebagging the head of a bag results in the old bag having a new head and an overall correct
	// state.
	#[test]
	fn rebag_head_works() {
		ExtBuilder::default().build_and_execute(|| {
			// when
			NextVoteWeight::set(10);
			assert_ok!(BagsList::rebag(Origin::signed(0), 2));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 2]), (1000, vec![3, 4])]);
			assert_eq!(Bag::<Runtime>::get(1_000).unwrap(), Bag::new(Some(3), Some(4), 1_000));

			// when
			assert_ok!(BagsList::rebag(Origin::signed(0), 3));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 2, 3]), (1000, vec![4])]);
			assert_eq!(Bag::<Runtime>::get(1_000).unwrap(), Bag::new(Some(4), Some(4), 1_000));

			// when
			assert_ok!(BagsList::rebag(Origin::signed(0), 4));

			// then
			assert_eq!(get_bags(), vec![(10, vec![1, 2, 3, 4])]);
			assert_eq!(Bag::<Runtime>::get(1_000), None);
		});
	}
}

mod sorted_list_provider {
	use super::*;

	#[test]
	fn iter_works() {
		ExtBuilder::default().build_and_execute(|| {
			let expected = vec![2, 3, 4, 1];
			for (i, id) in <BagsList as SortedListProvider<AccountId>>::iter().enumerate() {
				assert_eq!(id, expected[i])
			}
		});
	}

	#[test]
	fn count_works() {
		ExtBuilder::default().build_and_execute(|| {
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 4);

			<BagsList as SortedListProvider<AccountId>>::on_insert(201, 0);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);

			<BagsList as SortedListProvider<AccountId>>::on_remove(&201);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 4);

			<BagsList as SortedListProvider<AccountId>>::on_remove(&1);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 3);

			<BagsList as SortedListProvider<AccountId>>::on_remove(&2);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 2);
		});
	}

	#[test]
	fn on_insert_works() {
		ExtBuilder::default().build_and_execute(|| {
			// when
			<BagsList as SortedListProvider<AccountId>>::on_insert(71, 1_000);

			// then
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![2, 3, 4, 71])]);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![2, 3, 4, 71, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);
			assert_ok!(List::<Runtime>::sanity_check());

			// when
			List::<Runtime>::insert(81, 1_001);

			// then
			assert_eq!(
				get_bags(),
				vec![(10, vec![1]), (1_000, vec![2, 3, 4, 71]), (2000, vec![81])]
			);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![81, 2, 3, 4, 71, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 6);
		})
	}

	#[test]
	fn on_update_works() {
		ExtBuilder::default().add_ids(vec![(42, 20)]).build_and_execute(|| {
			// given
			assert_eq!(get_bags(), vec![(10, vec![1]), (20, vec![42]), (1_000, vec![2, 3, 4])]);

			// update weight to the level of non-existent bag
			<BagsList as SortedListProvider<AccountId>>::on_update(&42, 2_000);
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![2, 3, 4]), (2000, vec![42])]);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![42, 2, 3, 4, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);
			assert_ok!(List::<Runtime>::sanity_check());

			// decrease weight within the range of the current bag
			<BagsList as SortedListProvider<AccountId>>::on_update(&42, 1_001);
			// does not change bags
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![2, 3, 4]), (2000, vec![42])]);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![42, 2, 3, 4, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);
			assert_ok!(List::<Runtime>::sanity_check());

			// increase weight to the level of a non-existent bag
			<BagsList as SortedListProvider<AccountId>>::on_update(&42, VoteWeight::MAX);
			// creates the bag and moves the voter into it
			assert_eq!(
				get_bags(),
				vec![(10, vec![1]), (1_000, vec![2, 3, 4]), (VoteWeight::MAX, vec![42])]
			);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![42, 2, 3, 4, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);
			assert_ok!(List::<Runtime>::sanity_check());

			// decrease the weight to a pre-existing bag
			<BagsList as SortedListProvider<AccountId>>::on_update(&42, 999);
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![2, 3, 4, 42])]);
			assert_eq!(
				<BagsList as SortedListProvider<AccountId>>::iter().collect::<Vec<_>>(),
				vec![2, 3, 4, 42, 1]
			);
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), 5);
		});
	}

	#[test]
	fn on_remove_works() {
		let ensure_left = |id, counter| {
			assert!(!VoterBagFor::<Runtime>::contains_key(id));
			assert!(!VoterNodes::<Runtime>::contains_key(id));
			assert_eq!(<BagsList as SortedListProvider<AccountId>>::count(), counter);
			assert_eq!(CounterForVoters::<Runtime>::get(), counter);
			assert_eq!(VoterBagFor::<Runtime>::iter().count() as u32, counter);
			assert_eq!(VoterNodes::<Runtime>::iter().count() as u32, counter);
		};

		ExtBuilder::default().build_and_execute(|| {
			// when removing a non-existent voter
			assert!(!get_voter_list_as_ids().contains(&42));
			assert!(!VoterNodes::<Runtime>::contains_key(42));
			<BagsList as SortedListProvider<AccountId>>::on_remove(&42);

			// then nothing changes
			assert_eq!(get_voter_list_as_ids(), vec![2, 3, 4, 1]);
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![2, 3, 4])]);

			// when removing a node from a bag with multiple nodes
			<BagsList as SortedListProvider<AccountId>>::on_remove(&2);

			// then
			assert_eq!(get_voter_list_as_ids(), vec![3, 4, 1]);
			assert_eq!(get_bags(), vec![(10, vec![1]), (1_000, vec![3, 4])]);
			ensure_left(2, 3);

			// when removing a node from a bag with only one node:
			<BagsList as SortedListProvider<AccountId>>::on_remove(&1);

			// then
			assert_eq!(get_voter_list_as_ids(), vec![3, 4]);
			assert_eq!(get_bags(), vec![(1_000, vec![3, 4])]);
			ensure_left(1, 2);

			// remove remaining voters to make sure storage cleans up as expected
			<BagsList as SortedListProvider<AccountId>>::on_remove(&4);
			assert_eq!(get_voter_list_as_ids(), vec![3]);
			ensure_left(4, 1);

			<BagsList as SortedListProvider<AccountId>>::on_remove(&3);
			assert_eq!(get_voter_list_as_ids(), Vec::<AccountId>::new());
			ensure_left(3, 0);
		});
	}

	#[test]
	fn sanity_check_works() {
		ExtBuilder::default().build_and_execute_no_post_check(|| {
			assert_ok!(List::<Runtime>::sanity_check());
		});

		// make sure there are no duplicates.
		ExtBuilder::default().build_and_execute_no_post_check(|| {
			<BagsList as SortedListProvider<AccountId>>::on_insert(2, 10);
			assert_eq!(List::<Runtime>::sanity_check(), Err("duplicate identified".to_string()));
		});

		// ensure count is in sync with `CounterForVoters`.
		ExtBuilder::default().build_and_execute_no_post_check(|| {
			crate::CounterForVoters::<Runtime>::mutate(|counter| *counter += 1);
			assert_eq!(crate::CounterForVoters::<Runtime>::get(), 5);
			assert_eq!(
				List::<Runtime>::sanity_check(),
				Err("iter_count 4 != stored_count 5".to_string())
			);
		});
	}
}