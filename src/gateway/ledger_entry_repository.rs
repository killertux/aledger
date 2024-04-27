use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::{
    operation::transact_write_items::{
        builders::TransactWriteItemsFluentBuilder, TransactWriteItemsError,
    },
    types::{
        AttributeValue, Condition, Delete, KeysAndAttributes, Put,
        ReturnValuesOnConditionCheckFailure, TransactWriteItem, Update,
    },
    Client,
};
use chrono::{DateTime, Days, TimeDelta, Utc};
use itertools::Itertools;
use ulid::Ulid;
use uuid::Uuid;

use crate::domain::entity::AccountId;
use crate::domain::entity::Cursor;
use crate::domain::entity::LedgerBalanceName;
use crate::domain::entity::LedgerFieldName;
use crate::domain::entity::{Entry, EntryId, EntryStatus, EntryWithBalance};
use crate::domain::{
    entity::Order,
    gateway::{AppendEntriesError, GetBalanceError, LedgerEntryRepository, RevertEntriesError},
};

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
        account_id: &AccountId,
        entries: &[Entry],
    ) -> Result<Vec<EntryWithBalance>, AppendEntriesError> {
        let (transact, entries_with_balance) = self
            .internal_append_entries(account_id, entries, self.client.transact_write_items())
            .await?;

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
                                let mut entry_id = entry_id
                                    .as_s()
                                    .map_err(|_| anyhow!("Cannot read attribute as string"))?
                                    .as_str();
                                if entry_id == "HEAD" {
                                    return Err(AppendEntriesError::OptimisticLockError(
                                        account_id.clone(),
                                    ));
                                }
                                if let Some((n_entry_id, _revert_id)) = entry_id.split_once('|') {
                                    entry_id = n_entry_id;
                                }
                                entries.push(EntryId::new_unchecked(entry_id.to_owned()))
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

    async fn revert_entries(
        &self,
        account_id: &AccountId,
        entries_ids: &[EntryId],
    ) -> Result<Vec<EntryWithBalance>, RevertEntriesError> {
        let mut keys_and_attributes_builder = KeysAndAttributes::builder();

        for entry_id in entries_ids {
            keys_and_attributes_builder = keys_and_attributes_builder.keys(HashMap::from([
                ("pk".into(), AttributeValue::S(account_id.to_string())),
                (
                    "sk".into(),
                    AttributeValue::S(format!("{}|~", entry_id.to_string())),
                ),
            ]));
        }
        let items = self
            .client
            .batch_get_item()
            .request_items(
                "a_ledger",
                keys_and_attributes_builder
                    .build()
                    .map_err(anyhow::Error::from)?,
            )
            .send()
            .await
            .map_err(anyhow::Error::from)?;
        let mut entry_with_balances = items
            .responses()
            .and_then(|responses| responses.get("a_ledger"))
            .map(
                |responses| -> Result<HashMap<EntryId, EntryWithBalance>, GetBalanceError> {
                    Ok(responses
                        .iter()
                        .map(|item| {
                            let entry = entry_with_balance_from_item(item)?;
                            Ok((entry.entry_id.clone(), entry))
                        })
                        .collect::<Result<HashMap<EntryId, EntryWithBalance>, GetBalanceError>>()
                        .map_err(anyhow::Error::from)?)
                },
            )
            .transpose()
            .map_err(anyhow::Error::from)?
            .unwrap_or_default();

        let found_entries_ids: HashSet<EntryId> = entry_with_balances.keys().cloned().collect();
        let missing_entries = entries_ids
            .iter()
            .cloned()
            .collect::<HashSet<EntryId>>()
            .difference(&found_entries_ids)
            .cloned()
            .collect_vec();
        if !missing_entries.is_empty() {
            return Err(RevertEntriesError::EntriesDoesNotExists(
                account_id.clone(),
                missing_entries,
            ));
        }
        let (mut transact, new_entries_with_balance) = self
            .internal_append_entries(
                account_id,
                &entry_with_balances
                    .values()
                    .map(|entry| entry.clone().into())
                    .map(|mut entry: Entry| {
                        let status = EntryStatus::Reverts(entry.entry_id);
                        entry.entry_id = EntryId::new_unchecked(Ulid::new().to_string());
                        entry.status = status;
                        entry.ledger_fields = entry
                            .ledger_fields
                            .into_iter()
                            .map(|(key, value)| (key, -value))
                            .collect();
                        entry
                    })
                    .collect_vec(),
                self.client.transact_write_items(),
            )
            .await?;
        for entry in new_entries_with_balance.iter() {
            let EntryStatus::Reverts(entry_id) = &entry.status else {
                return Err(anyhow!("Expects status to be reverts").into());
            };
            let mut old_entry = entry_with_balances
                .remove(entry_id)
                .ok_or(anyhow!("We should alway be able to get the old entry here"))?;
            old_entry.status = EntryStatus::RevertedBy(entry.entry_id.clone());
            old_entry.entry_id = EntryId::new_unchecked(format!(
                "{}|{}",
                old_entry.entry_id.to_string(),
                entry.entry_id.to_string()
            ));
            transact = transact.transact_items(
                TransactWriteItem::builder()
                    .delete(
                        Delete::builder()
                            .table_name("a_ledger")
                            .key("pk", AttributeValue::S(account_id.to_string()))
                            .key(
                                "sk",
                                AttributeValue::S(format!("{}|~", entry_id.to_string())),
                            )
                            .build()
                            .map_err(anyhow::Error::from)?,
                    )
                    .build(),
            );
            transact = transact.transact_items(create_transact_item_for_entry(&old_entry, false)?);
        }

        match transact.send().await {
            Ok(_) => Ok(new_entries_with_balance),
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
                        for cancellation_reason in err.cancellation_reasons() {
                            if let Some(entry_id) =
                                cancellation_reason.item().and_then(|item| item.get("sk"))
                            {
                                let entry_id = entry_id
                                    .as_s()
                                    .map_err(|_| anyhow!("Cannot read attribute as string"))?;
                                if entry_id == "HEAD" {
                                    return Err(RevertEntriesError::OptimisticLockError(
                                        account_id.clone(),
                                    ));
                                }
                            }
                        }
                        return Err(anyhow::Error::from(error).into());
                    }
                }
                Err(anyhow::Error::from(error).into())
            }
        }
    }

    async fn get_balance(
        &self,
        account_id: &AccountId,
    ) -> Result<EntryWithBalance, GetBalanceError> {
        let item = self
            .client
            .get_item()
            .table_name("a_ledger")
            .key("pk", AttributeValue::S(account_id.to_string()))
            .key("sk", AttributeValue::S("HEAD".into()))
            .send()
            .await
            .map_err(anyhow::Error::from)?;
        match item.item {
            None => Err(GetBalanceError::NotFound(account_id.clone())),
            Some(item) => entry_with_balance_from_item(&item),
        }
    }

    async fn get_entry(
        &self,
        account_id: &AccountId,
        entry_id: &EntryId,
    ) -> Result<Vec<EntryWithBalance>, GetBalanceError> {
        let items = self
            .client
            .query()
            .limit(100)
            .table_name("a_ledger")
            .key_conditions(
                "pk",
                Condition::builder()
                    .comparison_operator(aws_sdk_dynamodb::types::ComparisonOperator::Eq)
                    .attribute_value_list(AttributeValue::S(account_id.to_string()))
                    .build()
                    .map_err(anyhow::Error::from)?,
            )
            .key_conditions(
                "sk",
                Condition::builder()
                    .comparison_operator(aws_sdk_dynamodb::types::ComparisonOperator::BeginsWith)
                    .attribute_value_list(AttributeValue::S(entry_id.to_string()))
                    .build()
                    .map_err(anyhow::Error::from)?,
            )
            .scan_index_forward(false)
            .send()
            .await
            .map_err(anyhow::Error::from)?;
        let entry_with_balances = items
            .items()
            .iter()
            .map(entry_with_balance_from_item)
            .collect::<Result<Vec<EntryWithBalance>, GetBalanceError>>()?;
        if entry_with_balances.is_empty() {
            return Err(GetBalanceError::NotFound(account_id.clone()));
        }
        Ok(entry_with_balances)
    }

    async fn get_entries(
        &self,
        account_id: &AccountId,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        limit: u8,
        order: &Order,
    ) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
        let start_naive_date = start_date.date_naive();
        let end_naive_date = end_date.date_naive();
        let mut current_date = match order {
            Order::Asc => start_naive_date,
            Order::Desc => end_naive_date,
        };
        let mut result = Vec::new();
        loop {
            let items = self
                .client
                .query()
                .limit((limit as usize - result.len()) as i32 + 1)
                .table_name("a_ledger")
                .index_name("a_ledger_created_at_idx")
                .key_conditions(
                    "account_id_and_date",
                    Condition::builder()
                        .comparison_operator(aws_sdk_dynamodb::types::ComparisonOperator::Eq)
                        .attribute_value_list(AttributeValue::S(format!(
                            "{}|{}",
                            account_id, current_date
                        )))
                        .build()
                        .map_err(anyhow::Error::from)?,
                )
                .key_conditions(
                    "created_at",
                    Condition::builder()
                        .comparison_operator(aws_sdk_dynamodb::types::ComparisonOperator::Between)
                        .attribute_value_list(AttributeValue::S(start_date.to_string()))
                        .attribute_value_list(AttributeValue::S(end_date.to_string()))
                        .build()
                        .map_err(anyhow::Error::from)?,
                )
                .filter_expression("sk <> :head")
                .expression_attribute_values(":head", AttributeValue::S("HEAD".into()))
                .scan_index_forward(*order == Order::Asc)
                .send()
                .await
                .map_err(anyhow::Error::from)?;
            let mut entry_with_balances = items
                .items()
                .iter()
                .map(entry_with_balance_from_item)
                .collect::<Result<Vec<EntryWithBalance>, GetBalanceError>>(
            )?;
            result.append(&mut entry_with_balances);

            if result.len() > limit as usize {
                break;
            }

            match order {
                Order::Asc => {
                    if current_date == end_naive_date {
                        break;
                    }
                    current_date = current_date
                        .checked_add_days(Days::new(1))
                        .ok_or(anyhow!("Failed to increment current_date"))?;
                }
                Order::Desc => {
                    if current_date == start_naive_date {
                        break;
                    }
                    current_date = current_date
                        .checked_sub_days(Days::new(1))
                        .ok_or(anyhow!("Failed to decrement current_date"))?;
                }
            }
        }
        result.drain((limit as usize).min(result.len())..result.len());

        let cursor = {
            if result.len() < limit as usize {
                None
            } else {
                Some(match order {
                    Order::Asc => Cursor::new(
                        result
                            .last()
                            .ok_or(anyhow!("Expects at least one entry in the vector"))?
                            .created_at
                            + TimeDelta::new(0, 1).ok_or(anyhow!("Time delta should be valid"))?,
                        *end_date,
                        order.clone(),
                        account_id.clone(),
                    ),
                    Order::Desc => Cursor::new(
                        *start_date,
                        result
                            .last()
                            .ok_or(anyhow!("Expects at least one entry in the vector"))?
                            .created_at
                            - TimeDelta::new(0, 1).ok_or(anyhow!("Time delta should be valid"))?,
                        order.clone(),
                        account_id.clone(),
                    ),
                })
            }
        };

        Ok((result, cursor))
    }
}

impl DynamoDbLedgerEntryRepository {
    async fn internal_append_entries(
        &self,
        account_id: &AccountId,
        entries: &[Entry],
        mut transact: TransactWriteItemsFluentBuilder,
    ) -> Result<(TransactWriteItemsFluentBuilder, Vec<EntryWithBalance>), AppendEntriesError> {
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
                    status: entry.status.clone(),
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
                    status: entry.status.clone(),
                    ledger_fields: entry.ledger_fields.clone(),
                    additional_fields: entry.additional_fields.clone(),
                    created_at: Utc::now(),
                },
            };
            entries_with_balance.push(new_entry);
        }
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
                                    ":status",
                                    AttributeValue::S(
                                        serde_json::to_string(&entry.status).map_err(anyhow::Error::from)?,
                                    ),
                                )
                                .expression_attribute_values(
                                    ":entry_id",
                                    AttributeValue::S(
                                        entry.entry_id.to_string(),
                                    ),
                                )
                                .expression_attribute_values(
                                    ":created_at",
                                    AttributeValue::S(
                                        entry.created_at.to_string(),
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
                                .update_expression("SET ledger_balances = :ledger_balances, ledger_fields = :ledger_fields, additional_fields = :additional_fields, entry_id = :entry_id, created_at = :created_at, entry_status = :status")
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
        Ok((transact, entries_with_balance))
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
                format!("{}|~", entry.entry_id.to_string())
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
            "account_id_and_date",
            AttributeValue::S(format!(
                "{}|{}",
                entry.account_id,
                entry.created_at.date_naive()
            )),
        )
        .item(
            "entry_status",
            AttributeValue::S(serde_json::to_string(&entry.status)?),
        )
        .item(
            "created_at",
            AttributeValue::S(entry.created_at.to_string()),
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

fn entry_with_balance_from_item(
    item: &HashMap<String, AttributeValue>,
) -> Result<EntryWithBalance, GetBalanceError> {
    let mut entry_id = item
        .get("entry_id")
        .or(item.get("sk"))
        .ok_or(GetBalanceError::MissingField("entry_id".into()))?
        .as_s()
        .map_err(|_| GetBalanceError::ErrorReadingField("entry_id".into()))?
        .as_str();
    if let Some((n_entry_id, _revert_id)) = entry_id.split_once('|') {
        entry_id = n_entry_id;
    }
    Ok(EntryWithBalance {
        account_id: AccountId::new(
            Uuid::from_str(
                item.get("pk")
                    .ok_or(GetBalanceError::MissingField("pk".into()))?
                    .as_s()
                    .map_err(|_| GetBalanceError::ErrorReadingField("pk".into()))?,
            )
            .map_err(|_| GetBalanceError::ErrorReadingField("pk".into()))?,
        ),
        entry_id: EntryId::new_unchecked(entry_id.to_owned()),
        ledger_balances: item
            .get("ledger_balances")
            .ok_or(GetBalanceError::MissingField("ledger_balances".into()))?
            .as_m()
            .map_err(|_| GetBalanceError::ErrorReadingField("ledger_balances".into()))?
            .iter()
            .map(|(k, v)| {
                Ok((
                    LedgerBalanceName::new(k.clone()).map_err(|_| {
                        GetBalanceError::ErrorReadingField("ledger_balances".into())
                    })?,
                    v.as_n()
                        .map_err(|_| GetBalanceError::ErrorReadingField("ledger_balances".into()))?
                        .parse::<i128>()
                        .map_err(|_| {
                            GetBalanceError::ErrorReadingField("ledger_balances".into())
                        })?,
                ))
            })
            .collect::<Result<HashMap<LedgerBalanceName, i128>>>()?,
        ledger_fields: item
            .get("ledger_fields")
            .ok_or(GetBalanceError::MissingField("ledger_fields".into()))?
            .as_m()
            .map_err(|_| GetBalanceError::ErrorReadingField("ledger_fields".into()))?
            .iter()
            .map(|(k, v)| {
                Ok((
                    LedgerFieldName::new(k.clone())
                        .map_err(|_| GetBalanceError::ErrorReadingField("ledger_fields".into()))?,
                    v.as_n()
                        .map_err(|_| GetBalanceError::ErrorReadingField("ledger_fields".into()))?
                        .parse::<i128>()
                        .map_err(|_| GetBalanceError::ErrorReadingField("ledger_fields".into()))?,
                ))
            })
            .collect::<Result<HashMap<LedgerFieldName, i128>>>()?,
        additional_fields: serde_json::from_str(
            item.get("additional_fields")
                .ok_or(GetBalanceError::MissingField("additional_fields".into()))?
                .as_s()
                .map_err(|_| GetBalanceError::ErrorReadingField("additional_fields".into()))?,
        )
        .map_err(|_| GetBalanceError::ErrorReadingField("additional_fields".into()))?,
        status: serde_json::from_str(
            item.get("entry_status")
                .ok_or(GetBalanceError::MissingField("entry_status".into()))?
                .as_s()
                .map_err(|_| GetBalanceError::ErrorReadingField("entry_status".into()))?,
        )
        .map_err(|_| GetBalanceError::ErrorReadingField("entry_status".into()))?,
        created_at: DateTime::from_str(
            item.get("created_at")
                .ok_or(GetBalanceError::MissingField("created_at".into()))?
                .as_s()
                .map_err(|_| GetBalanceError::ErrorReadingField("created_at".into()))?,
        )
        .map_err(|_| GetBalanceError::ErrorReadingField("created_at".into()))?,
    })
}
