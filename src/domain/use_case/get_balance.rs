use crate::domain::entity::AccountId;
use crate::domain::entity::EntryWithBalance;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_balance_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
) -> Result<EntryWithBalance, GetBalanceError> {
    repository.get_balance(account_id).await
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use fake::{Fake, Faker};

    use crate::app::test::get_repository;
    use crate::domain::use_case::push_entries::test::push_multiple_entries;

    use super::*;

    #[tokio_shared_rt::test(shared)]
    async fn get_balance_from_nonexistent_account() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();

        assert_eq!(
            format!("Account not found with id `{0}`", account_id),
            get_balance_use_case(&repository, &account_id)
                .await
                .expect_err("Expect and error")
                .to_string()
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_balance() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let entries = push_multiple_entries(&repository, &account_id, 3).await;

        assert_eq!(
            *entries
                .last()
                .expect("We know that the vector is not empty"),
            get_balance_use_case(&repository, &account_id).await?
        );
        Ok(())
    }
}
