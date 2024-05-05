use anyhow::anyhow;
use chrono::{DateTime, Utc};

use crate::domain::entity::AccountId;
use crate::domain::entity::Cursor;
use crate::domain::entity::EntryWithBalance;
use crate::domain::entity::Order;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_entries_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
    limit: u8,
    order: &Order,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    repository
        .get_entries(account_id, start_date, end_date, limit, order, None)
        .await
}

pub async fn get_entries_from_cursor_use_case(
    repository: &impl LedgerEntryRepository,
    cursor: Cursor,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    let Cursor::FromEntriesQuery {
        start_date,
        end_date,
        order,
        account_id,
        sequence,
    } = cursor
    else {
        return Err(GetBalanceError::Other(anyhow!("Invalid cursor")));
    };
    repository
        .get_entries(
            &account_id,
            &start_date,
            &end_date,
            limit,
            &order,
            Some(sequence),
        )
        .await
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use fake::{Fake, Faker};

    use crate::app::test::get_repository;
    use crate::domain::entity::{Cursor, Order};
    use crate::domain::use_case::get_entries_use_case;
    use crate::domain::use_case::push_entries::test::{
        push_entry_with_date, push_multiple_entries, push_multiple_entries_with_date_interval,
    };
    use crate::utils::utc_now;

    use super::*;

    #[tokio_shared_rt::test(shared)]
    async fn get_entries_from_nonexistent_account() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();

        assert_eq!(
            (Vec::new(), None),
            get_entries_use_case(
                &repository,
                &account_id,
                &utc_now(),
                &utc_now(),
                10,
                &Order::Asc
            )
            .await?
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entries_asc() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let entries_with_balance = push_multiple_entries(&repository, &account_id, 5).await;

        assert_eq!(
            (entries_with_balance, None),
            get_entries_use_case(
                &repository,
                &account_id,
                &utc_now(),
                &utc_now(),
                10,
                &Order::Asc
            )
            .await?
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entries_desc() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let mut entries_with_balance = push_multiple_entries(&repository, &account_id, 5).await;
        entries_with_balance.reverse();

        assert_eq!(
            (entries_with_balance, None),
            get_entries_use_case(
                &repository,
                &account_id,
                &utc_now(),
                &utc_now(),
                10,
                &Order::Desc
            )
            .await?
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entries_multiple_days() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let _before_1 = push_entry_with_date(
            &repository,
            &account_id,
            &"2024-05-01 12:00:00 UTC".parse()?,
        )
        .await;
        let _before_2 = push_entry_with_date(
            &repository,
            &account_id,
            &"2024-05-02 12:00:00 UTC".parse()?,
        )
        .await;
        let entry_1 = push_entry_with_date(
            &repository,
            &account_id,
            &"2024-05-02 12:00:01 UTC".parse()?,
        )
        .await;
        let entry_2 = push_entry_with_date(
            &repository,
            &account_id,
            &"2024-05-03 12:00:02 UTC".parse()?,
        )
        .await;
        let _after_1 = push_entry_with_date(
            &repository,
            &account_id,
            &"2024-05-03 12:00:03 UTC".parse()?,
        )
        .await;
        assert_eq!(
            vec![entry_1.clone(), entry_2.clone()],
            get_entries_use_case(
                &repository,
                &account_id,
                &"2024-05-02 12:00:01 UTC".parse()?,
                &"2024-05-03 12:00:02 UTC".parse()?,
                10,
                &Order::Asc
            )
            .await?
            .0
        );
        assert_eq!(
            vec![entry_2.clone(), entry_1.clone()],
            get_entries_use_case(
                &repository,
                &account_id,
                &"2024-05-02 12:00:01 UTC".parse()?,
                &"2024-05-03 12:00:02 UTC".parse()?,
                10,
                &Order::Desc
            )
            .await?
            .0
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn number_of_entries_equals_to_limit_should_return_cursor() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let entries_with_balance = push_multiple_entries(&repository, &account_id, 5).await;

        let result = get_entries_use_case(
            &repository,
            &account_id,
            &utc_now(),
            &utc_now(),
            5,
            &Order::Asc,
        )
        .await?;
        assert_eq!(
            (
                entries_with_balance,
                Some(Cursor::FromEntriesQuery {
                    account_id: account_id.clone(),
                    start_date: utc_now(),
                    end_date: utc_now(),
                    sequence: 4,
                    order: Order::Asc,
                })
            ),
            result
        );
        assert_eq!(
            (Vec::new(), None),
            get_entries_from_cursor_use_case(&repository, result.1.unwrap(), 5).await?
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn asc_cursor() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let start_date = utc_now();
        let entries_with_balance =
            push_multiple_entries_with_date_interval(&repository, &account_id, 5).await;
        let (first_entries, second_entries) = entries_with_balance.split_at(3);
        let end_date = utc_now();

        let result = get_entries_use_case(
            &repository,
            &account_id,
            &start_date,
            &end_date,
            3,
            &Order::Asc,
        )
        .await?;
        assert_eq!(
            (
                first_entries.to_vec(),
                Some(Cursor::FromEntriesQuery {
                    account_id: account_id.clone(),
                    start_date: first_entries
                        .last()
                        .expect("We know the vector is not empty")
                        .created_at
                        .clone(),
                    end_date: end_date,
                    sequence: 2,
                    order: Order::Asc,
                })
            ),
            result
        );
        assert_eq!(
            (second_entries.to_vec(), None),
            get_entries_from_cursor_use_case(&repository, result.1.unwrap(), 5).await?
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn desc_cursor() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let start_date = utc_now();
        let mut entries_with_balance =
            push_multiple_entries_with_date_interval(&repository, &account_id, 5).await;
        entries_with_balance.reverse();
        let (first_entries, second_entries) = entries_with_balance.split_at(3);
        let end_date = utc_now();

        let result = get_entries_use_case(
            &repository,
            &account_id,
            &start_date,
            &end_date,
            3,
            &Order::Desc,
        )
        .await?;
        assert_eq!(
            (
                first_entries.to_vec(),
                Some(Cursor::FromEntriesQuery {
                    account_id: account_id.clone(),
                    start_date: start_date,
                    end_date: first_entries
                        .last()
                        .expect("We know the vector is not empty")
                        .created_at
                        .clone(),
                    sequence: 2,
                    order: Order::Desc,
                })
            ),
            result
        );
        assert_eq!(
            (second_entries.to_vec(), None),
            get_entries_from_cursor_use_case(&repository, result.1.unwrap(), 5).await?
        );
        Ok(())
    }
}
