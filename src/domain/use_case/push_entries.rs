use std::time::Duration;

use itertools::Itertools;
use rand::Rng;
use tokio::time::sleep;

use crate::domain::entity::{Entry, EntryWithBalance};
use crate::domain::gateway::{AppendEntriesError, LedgerEntryRepository};
use crate::domain::use_case;
use crate::domain::use_case::NonAppliedReason;

pub async fn push_entries_use_case(
    repository: &impl LedgerEntryRepository,
    mut random_number_generator: impl Rng,
    entries: impl Iterator<Item = Entry> + Send + Sync,
) -> (Vec<EntryWithBalance>, Vec<(NonAppliedReason, Entry)>) {
    let entries_by_account_id = entries.into_group_map_by(|v| v.account_id.clone());
    let mut applied_entries_with_balance = Vec::new();
    let mut non_applied_entries = Vec::new();

    for (account_id, total_entries) in entries_by_account_id.into_iter() {
        for entries in total_entries.chunks(99) {
            let mut entries = Vec::from(entries);
            let mut tries = 0;
            loop {
                tries += 1;
                match repository.append_entries(&account_id, &entries).await {
                    Ok(applied) => {
                        applied_entries_with_balance.extend(applied);
                        break;
                    }
                    Err(AppendEntriesError::OptimisticLockError(_)) if tries != 5 => {
                        if tries == 1 {
                            continue;
                        }
                        sleep(Duration::from_millis(
                            random_number_generator.gen_range(10..100),
                        ))
                        .await;
                    }
                    Err(AppendEntriesError::EntriesAlreadyExists(_, duplicated_entries_ids)) => {
                        let duplicated_entries = use_case::extract_if(&mut entries, |entry| {
                            duplicated_entries_ids.contains(&entry.entry_id)
                        });
                        non_applied_entries.extend(
                            duplicated_entries
                                .into_iter()
                                .map(|entry| (NonAppliedReason::EntriesAlreadyExists, entry)),
                        );
                    }
                    Err(err) => {
                        non_applied_entries.extend(entries.into_iter().map(|entry| {
                            (NonAppliedReason::from_append_entries_error(&err), entry)
                        }));
                        break;
                    }
                }
            }
        }
    }
    non_applied_entries.dedup();
    (applied_entries_with_balance, non_applied_entries)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::domain::{
        entity::AccountId,
        use_case::test::{get_repository, get_rng, EntryBuilder, EntryWithBalanceBuilder},
    };
    use anyhow::Result;
    use assertables::*;
    use fake::{Fake, Faker};

    #[tokio_shared_rt::test(shared)]
    async fn push_single_entry() -> Result<()> {
        let repository = get_repository().await;
        let rng = get_rng().await;
        let account_id: AccountId = Faker.fake();
        let entry = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", 100)
            .with_ledger_field("usd_amount", 301)
            .build();

        let (applied, non_applied) =
            push_entries_use_case(&repository, rng, [entry.clone()].into_iter()).await;
        assert!(non_applied.is_empty());
        assert_eq!(
            Vec::from([EntryWithBalanceBuilder::from_entry(entry)
                .with_ledger_balance("balance_local_amount", 100)
                .with_ledger_balance("balance_usd_amount", 301)
                .build()]),
            applied
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn push_multiple_entry_from_same_account_id() -> Result<()> {
        let repository = get_repository().await;
        let rng = get_rng().await;
        let account_id: AccountId = Faker.fake();
        let entry_1 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", 100)
            .with_ledger_field("usd_amount", 301)
            .build();
        let entry_2 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", -50)
            .with_ledger_field("usd_amount", -152)
            .build();
        let (applied, non_applied) = push_entries_use_case(
            &repository,
            rng,
            [entry_1.clone(), entry_2.clone()].into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        assert_eq!(
            Vec::from([
                EntryWithBalanceBuilder::from_entry(entry_1)
                    .with_ledger_balance("balance_local_amount", 100)
                    .with_ledger_balance("balance_usd_amount", 301)
                    .build(),
                EntryWithBalanceBuilder::from_entry(entry_2)
                    .with_ledger_balance("balance_local_amount", 50)
                    .with_ledger_balance("balance_usd_amount", 149)
                    .build(),
            ]),
            applied
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn push_multiple_entry_from_different_account_ids() -> Result<()> {
        let repository = get_repository().await;
        let rng = get_rng().await;
        let account_id_1: AccountId = Faker.fake();
        let account_id_2: AccountId = Faker.fake();
        let entry_1 = EntryBuilder::new()
            .with_account_id(account_id_1.clone())
            .with_ledger_field("local_amount", 100)
            .with_ledger_field("usd_amount", 301)
            .build();
        let entry_2 = EntryBuilder::new()
            .with_account_id(account_id_1.clone())
            .with_ledger_field("local_amount", -50)
            .with_ledger_field("usd_amount", -152)
            .build();
        let entry_3 = EntryBuilder::new()
            .with_account_id(account_id_2.clone())
            .with_ledger_field("local_amount", 123100)
            .with_ledger_field("usd_amount", 41233123)
            .with_ledger_field("another_amount", 33313)
            .build();
        let entry_4 = EntryBuilder::new()
            .with_account_id(account_id_2.clone())
            .with_ledger_field("local_amount", 12233)
            .with_ledger_field("usd_amount", 44412)
            .with_ledger_field("another_amount", 3312)
            .build();
        let (applied, non_applied) = push_entries_use_case(
            &repository,
            rng,
            [
                entry_1.clone(),
                entry_2.clone(),
                entry_3.clone(),
                entry_4.clone(),
            ]
            .into_iter(),
        )
        .await;
        assert!(dbg!(non_applied).is_empty());
        assert_eq!(4, applied.len());
        assert_contains!(
            applied,
            &EntryWithBalanceBuilder::from_entry(entry_1.clone())
                .with_ledger_balance("balance_local_amount", 100)
                .with_ledger_balance("balance_usd_amount", 301)
                .build()
        );
        assert_contains!(
            applied,
            &EntryWithBalanceBuilder::from_entry(entry_2.clone())
                .with_ledger_balance("balance_local_amount", 50)
                .with_ledger_balance("balance_usd_amount", 149)
                .build()
        );

        assert_contains!(
            applied,
            &EntryWithBalanceBuilder::from_entry(entry_3.clone())
                .with_ledger_balance("balance_local_amount", 123100)
                .with_ledger_balance("balance_usd_amount", 41233123)
                .with_ledger_balance("balance_another_amount", 33313)
                .build()
        );
        assert_contains!(
            applied,
            &EntryWithBalanceBuilder::from_entry(entry_4.clone())
                .with_ledger_balance("balance_local_amount", 135333)
                .with_ledger_balance("balance_usd_amount", 41277535)
                .with_ledger_balance("balance_another_amount", 36625)
                .build()
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn push_duplicated_entry_in_same_request_should_not_apply() -> Result<()> {
        let repository = get_repository().await;
        let rng = get_rng().await;
        let account_id: AccountId = Faker.fake();
        let entry_1 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", 100)
            .with_ledger_field("usd_amount", 301)
            .build();
        let entry_2 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", -50)
            .with_ledger_field("usd_amount", -152)
            .build();
        let (applied, non_applied) = push_entries_use_case(
            &repository,
            rng,
            [entry_2.clone(), entry_1.clone(), entry_2.clone()].into_iter(),
        )
        .await;
        assert_eq!(
            Vec::from([(NonAppliedReason::EntriesAlreadyExists, entry_2)]),
            non_applied
        );
        assert_eq!(
            Vec::from([EntryWithBalanceBuilder::from_entry(entry_1)
                .with_ledger_balance("balance_local_amount", 100)
                .with_ledger_balance("balance_usd_amount", 301)
                .build(),]),
            applied
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn push_duplicated_entry_in_different_request_should_not_apply() -> Result<()> {
        let repository = get_repository().await;
        let account_id: AccountId = Faker.fake();
        let entry_1 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", 100)
            .with_ledger_field("usd_amount", 301)
            .build();
        let entry_2 = EntryBuilder::new()
            .with_account_id(account_id.clone())
            .with_ledger_field("local_amount", -50)
            .with_ledger_field("usd_amount", -152)
            .build();
        let (applied_1, non_applied_1) = push_entries_use_case(
            &repository,
            get_rng().await,
            [entry_1.clone(), entry_2.clone()].into_iter(),
        )
        .await;
        let (applied_2, non_applied_2) =
            push_entries_use_case(&repository, get_rng().await, [entry_2.clone()].into_iter())
                .await;
        assert!(non_applied_1.is_empty());
        assert_eq!(
            Vec::from([
                EntryWithBalanceBuilder::from_entry(entry_1)
                    .with_ledger_balance("balance_local_amount", 100)
                    .with_ledger_balance("balance_usd_amount", 301)
                    .build(),
                EntryWithBalanceBuilder::from_entry(entry_2.clone())
                    .with_ledger_balance("balance_local_amount", 50)
                    .with_ledger_balance("balance_usd_amount", 149)
                    .build(),
            ]),
            applied_1
        );
        assert!(applied_2.is_empty());
        assert_eq!(
            Vec::from([(NonAppliedReason::EntriesAlreadyExists, entry_2)]),
            non_applied_2
        );

        Ok(())
    }
}
