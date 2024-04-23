use crate::domain::{
    entity::{EntryId, EntryWithBalance, LedgerBalanceName},
    gateway::{AppendEntriesError, LedgerEntryRepository},
};
use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::{
    operation::transact_write_items::TransactWriteItemsError,
    types::{AttributeValue, Put, ReturnValuesOnConditionCheckFailure, TransactWriteItem, Update},
    Client,
};
use chrono::Utc;
use std::collections::HashMap;

pub struct DynamoDbLedgerEntryRepository {
    client: Client,
}

impl From<Client> for DynamoDbLedgerEntryRepository {
    fn from(client: Client) -> Self {
        Self { client }
    }
}

impl LedgerEntryRepository for DynamoDbLedgerEntryRepository {
    async fn append_entries(
        &self,
        account_id: &crate::domain::entity::AccountId,
        entries: &[crate::domain::entity::Entry],
    ) -> Result<
        Vec<crate::domain::entity::EntryWithBalance>,
        crate::domain::gateway::AppendEntriesError,
    > {
        let head_balances = self
            .client
            .get_item()
            .table_name("a_ledger")
            .key("pk", AttributeValue::S(account_id.to_string()))
            .key("sk", AttributeValue::S("HEAD".into()))
            .send()
            .await
            .map_err(anyhow::Error::from)?
            .item()
            .map(|item| -> Result<HashMap<LedgerBalanceName, i128>> {
                item.get("ledger_balances")
                    .ok_or(anyhow!(
                        "Missing ledger_balances for HEAD of account_id {}",
                        account_id.to_string()
                    ))?
                    .as_m()
                    .map_err(|_| anyhow!("Not a map"))?
                    .iter()
                    .map(|(k, v)| -> Result<(LedgerBalanceName, i128)> {
                        Ok((
                            LedgerBalanceName::new(k.clone())?,
                            v.as_n()
                                .map_err(|_| anyhow!("Not a number"))?
                                .parse::<i128>()?,
                        ))
                    })
                    .collect::<Result<HashMap<LedgerBalanceName, i128>>>()
            })
            .transpose()?;
        let mut entries_with_balance: Vec<EntryWithBalance> = Vec::new();
        for entry in entries {
            let new_entry = match entries_with_balance.last() {
                Some(entry_with_balance) => EntryWithBalance {
                    account_id: entry.account_id.clone(),
                    entry_id: entry.entry_id.clone(),
                    ledger_balances: entry
                        .ledger_fields
                        .iter()
                        .map(|(field_name, value)| {
                            let ledger_balance_name = LedgerBalanceName::from(field_name.clone());
                            let balance = entry_with_balance
                                .ledger_balances
                                .get(&ledger_balance_name)
                                .unwrap_or(&0);
                            let new_balance = balance + value;
                            (ledger_balance_name, new_balance)
                        })
                        .collect(),
                    ledger_fields: entry.ledger_fields.clone(),
                    additional_fields: entry.additional_fields.clone(),
                    created_at: Utc::now(),
                },
                None => EntryWithBalance {
                    account_id: entry.account_id.clone(),
                    entry_id: entry.entry_id.clone(),
                    ledger_balances: entry
                        .ledger_fields
                        .iter()
                        .map(|(field_name, value)| {
                            let ledger_balance_name = LedgerBalanceName::from(field_name.clone());
                            let balance = head_balances
                                .as_ref()
                                .and_then(|balances| balances.get(&ledger_balance_name).cloned())
                                .unwrap_or(0);
                            let new_balance = balance + value;
                            (ledger_balance_name, new_balance)
                        })
                        .collect(),
                    ledger_fields: entry.ledger_fields.clone(),
                    additional_fields: entry.additional_fields.clone(),
                    created_at: Utc::now(),
                },
            };
            entries_with_balance.push(new_entry);
        }
        let mut transact = self.client.transact_write_items();
        for entry in entries_with_balance.iter() {
            transact = transact.transact_items(create_transact_item_for_entry(entry, false)?);
        }
        match head_balances {
            Some(balance) => {
                let entry = entries_with_balance.last().ok_or(anyhow!(
                    "Missing last entry for account_id {}",
                    account_id.to_string()
                ))?;
                transact = transact.transact_items(
                    TransactWriteItem::builder()
                        .update(
                            Update::builder()
                                .table_name("a_ledger")
                                .key("pk", AttributeValue::S(account_id.to_string()))
                                .key("sk", AttributeValue::S("HEAD".into()))
                                .expression_attribute_values(
                                    ":ledger_balances",
                                    AttributeValue::M(
                                        entry
                                            .ledger_balances
                                            .clone()
                                            .into_iter()
                                            .map(|(k, v)| (k.into(), AttributeValue::N(v.to_string())))
                                            .collect(),
                                    ),
                                )
                                .expression_attribute_values(
                                    ":ledger_fields",
                                    AttributeValue::M(
                                        entry
                                            .ledger_fields
                                            .clone()
                                            .into_iter()
                                            .map(|(k, v)| (k.into(), AttributeValue::N(v.to_string())))
                                            .collect(),
                                    ),
                                )
                                .expression_attribute_values(
                                    ":additional_fields",
                                    AttributeValue::S(
                                        serde_json::to_string(&entry.additional_fields).map_err(anyhow::Error::from)?,
                                    ),
                                )
                                .expression_attribute_values(
                                    ":entry_id",
                                    AttributeValue::S(
                                        entry.entry_id.to_string(),
                                    ),
                                )
                                .expression_attribute_values(
                                    ":old_ledger_balances",
                                    AttributeValue::M(
                                        balance
                                            .into_iter()
                                            .map(|(k, v)| (k.into(), AttributeValue::N(v.to_string())))
                                            .collect(),
                                    ),
                                )
                                .update_expression("SET ledger_balances = :ledger_balances, ledger_fields = :ledger_fields, additional_fields = :additional_fields, entry_id = :entry_id")
                                .condition_expression("ledger_balances = :old_ledger_balances")
                                .return_values_on_condition_check_failure(
                                    ReturnValuesOnConditionCheckFailure::AllOld,
                                )
                                .build()
                                .map_err(anyhow::Error::from)?,
                        )
                        .build(),
                );
            }
            None => {
                transact = transact.transact_items(create_transact_item_for_entry(
                    entries_with_balance.last().ok_or(anyhow!(
                        "Missing last entry for account_id {}",
                        account_id.to_string()
                    ))?,
                    true,
                )?);
            }
        }
        match transact.send().await {
            Ok(_) => Ok(entries_with_balance),
            Err(error) => {
                if let Some(TransactWriteItemsError::TransactionCanceledException(err)) =
                    error.as_service_error()
                {
                    if err
                        .message
                        .as_ref()
                        .map(|msg| msg.contains("ConditionalCheckFailed"))
                        .unwrap_or(false)
                    {
                        let mut entries = Vec::new();
                        for cancellation_reason in err.cancellation_reasons() {
                            if let Some(entry_id) =
                                cancellation_reason.item().and_then(|item| item.get("sk"))
                            {
                                let entry_id = entry_id
                                    .as_s()
                                    .map_err(|_| anyhow!("Cannot read attribute as string"))?;
                                if entry_id == "HEAD" {
                                    return Err(AppendEntriesError::OptimisticLockError(
                                        account_id.clone(),
                                    ));
                                }
                                entries.push(EntryId::new(entry_id.to_owned())?)
                            }
                        }
                        return Err(AppendEntriesError::EntriesAlreadyExists(
                            account_id.clone(),
                            entries,
                        ));
                    }
                }
                Err(anyhow::Error::from(error).into())
            }
        }
    }
}

fn create_transact_item_for_entry(
    entry: &EntryWithBalance,
    is_head: bool,
) -> Result<TransactWriteItem> {
    let mut put_builder = Put::builder()
        .table_name("a_ledger")
        .item("pk", AttributeValue::S(entry.account_id.to_string()))
        .item(
            "sk",
            AttributeValue::S(if is_head {
                "HEAD".into()
            } else {
                entry.entry_id.to_string()
            }),
        )
        .item(
            "ledger_balances",
            AttributeValue::M(
                entry
                    .ledger_balances
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k.into(), AttributeValue::N(v.to_string())))
                    .collect(),
            ),
        )
        .item(
            "ledger_fields",
            AttributeValue::M(
                entry
                    .ledger_fields
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k.into(), AttributeValue::N(v.to_string())))
                    .collect(),
            ),
        )
        .item(
            "additional_fields",
            AttributeValue::S(serde_json::to_string(&entry.additional_fields)?),
        )
        .item(
            "created_at",
            AttributeValue::S(if is_head {
                Utc::now().to_string()
            } else {
                entry.created_at.to_string()
            }),
        )
        .condition_expression("attribute_not_exists(pk)")
        .return_values_on_condition_check_failure(ReturnValuesOnConditionCheckFailure::AllOld);
    if is_head {
        put_builder = put_builder.item("entry_id", AttributeValue::S(entry.entry_id.to_string()));
    }
    Ok(TransactWriteItem::builder()
        .put(put_builder.build()?)
        .build())
}
