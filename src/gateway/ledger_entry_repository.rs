use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use anyhow::{anyhow, bail, Result};
use aws_sdk_dynamodb::types::ComparisonOperator;
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
use chrono::{DateTime, Days, Utc};
use itertools::Itertools;
use uuid::Uuid;

use crate::domain::{
    entity::{
        AccountId, Entry, EntryId, EntryStatus, EntryToContinue, EntryWithBalance,
        LedgerBalanceName, LedgerFieldName, Order,
    },
    gateway::{AppendEntriesError, GetBalanceError, LedgerEntryRepository, RevertEntriesError},
};
use crate::{domain::entity::Cursor, utils::utc_now};

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
                            if let Some(pk) =
                                cancellation_reason.item().and_then(|item| item.get("pk"))
                            {
                                let pk = Pk::try_from(pk.clone())?;
                                match pk {
                                    Pk::Balance(account_id) => {
                                        return Err(AppendEntriesError::OptimisticLockError(
                                            account_id,
                                        ))
                                    }
                                    Pk::Entry(_, entry_id) => entries.push(entry_id),
                                }
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
                (
                    "pk".into(),
                    Pk::Entry(account_id.clone(), entry_id.clone()).into(),
                ),
                ("sk".into(), Sk::CurrentEntry.into()),
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
                &entries_ids
                    .iter()
                    .filter_map(|entry_id| entry_with_balances.get(entry_id).cloned())
                    .map(|entry: EntryWithBalance| {
                        let sequence = entry.sequence;
                        let mut entry: Entry = entry.into();
                        entry.status = EntryStatus::Revert(sequence);
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
            let EntryStatus::Revert(sequence) = &entry.status else {
                return Err(anyhow!("Expects status to be revert").into());
            };
            let mut old_entry = entry_with_balances
                .remove(
                    &entry_with_balances
                        .iter()
                        .find(|(_, entry_with_balance)| entry_with_balance.sequence == *sequence)
                        .ok_or(anyhow!("We should alway be able to get the old entry here"))?
                        .0
                        .clone(),
                )
                .ok_or(anyhow!("We should alway be able to get the old entry here"))?;
            old_entry.status = EntryStatus::Reverted(entry.sequence);
            transact = transact.transact_items(create_transact_item_for_entry(&old_entry, false)?);
            transact = transact.transact_items(
                TransactWriteItem::builder()
                    .delete(
                        Delete::builder()
                            .table_name("a_ledger")
                            .key(
                                "pk",
                                Pk::Entry(account_id.clone(), old_entry.entry_id.clone()).into(),
                            )
                            .key("sk", Sk::CurrentEntry.into())
                            .build()
                            .map_err(anyhow::Error::from)?,
                    )
                    .build(),
            );
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
                            if let Some(pk) =
                                cancellation_reason.item().and_then(|item| item.get("pk"))
                            {
                                let pk = Pk::try_from(pk.clone())?;
                                if let Pk::Balance(account_id) = pk {
                                    return Err(RevertEntriesError::OptimisticLockError(
                                        account_id,
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
            .key("pk", Pk::Balance(account_id.clone()).into())
            .key("sk", Sk::CurrentEntry.into())
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
        entry_to_continue: EntryToContinue,
        limit: u8,
    ) -> Result<Vec<EntryWithBalance>, GetBalanceError> {
        let sk_condition = match &entry_to_continue {
            EntryToContinue::Start => Condition::builder()
                .comparison_operator(ComparisonOperator::BeginsWith)
                .attribute_value_list(AttributeValue::S("|".into()))
                .build()
                .map_err(anyhow::Error::from)?,
            EntryToContinue::CurrentEntry => Condition::builder()
                .comparison_operator(ComparisonOperator::Lt)
                .attribute_value_list(AttributeValue::S("|~".into()))
                .build()
                .map_err(anyhow::Error::from)?,
            EntryToContinue::RevertedBy(sequence) => Condition::builder()
                .comparison_operator(ComparisonOperator::Lt)
                .attribute_value_list(Sk::RevertedEntry(*sequence).into())
                .build()
                .map_err(anyhow::Error::from)?,
        };
        let items = self
            .client
            .query()
            .limit(limit as i32)
            .table_name("a_ledger")
            .key_conditions(
                "pk",
                Condition::builder()
                    .comparison_operator(ComparisonOperator::Eq)
                    .attribute_value_list(Pk::Entry(account_id.clone(), entry_id.clone()).into())
                    .build()
                    .map_err(anyhow::Error::from)?,
            )
            .key_conditions("sk", sk_condition)
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
            if let EntryToContinue::Start = entry_to_continue {
                return Err(GetBalanceError::NotFound(account_id.clone()));
            }
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
        sequence: Option<u64>,
    ) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
        let start_naive_date = start_date.date_naive();
        let end_naive_date = end_date.date_naive();
        let mut current_date = match order {
            Order::Asc => start_naive_date,
            Order::Desc => end_naive_date,
        };
        let mut result = Vec::new();
        loop {
            let query_builder = self
                .client
                .query()
                .limit((limit as usize - result.len()) as i32 + 1)
                .table_name("a_ledger")
                .index_name("a_ledger_created_at_idx")
                .key_conditions(
                    "account_id_and_date",
                    Condition::builder()
                        .comparison_operator(ComparisonOperator::Eq)
                        .attribute_value_list(AttributeValue::S(format!(
                            "{}|{}",
                            account_id, current_date
                        )))
                        .build()
                        .map_err(anyhow::Error::from)?,
                );
            let query_builder = match order {
                Order::Asc => query_builder.key_conditions(
                    "created_at",
                    Condition::builder()
                        .comparison_operator(ComparisonOperator::Between)
                        .attribute_value_list(AttributeValue::S(if let Some(sequence) = sequence {
                            format_created_at_and_sequence(start_date, sequence + 1)
                        } else {
                            start_date.to_string()
                        }))
                        .attribute_value_list(AttributeValue::S(format_created_at_and_sequence(
                            end_date,
                            u64::MAX,
                        )))
                        .build()
                        .map_err(anyhow::Error::from)?,
                ),
                Order::Desc => query_builder.key_conditions(
                    "created_at",
                    Condition::builder()
                        .comparison_operator(ComparisonOperator::Between)
                        .attribute_value_list(AttributeValue::S(start_date.to_string()))
                        .attribute_value_list(AttributeValue::S(format_created_at_and_sequence(
                            end_date,
                            sequence.map(|sequence| sequence - 1).unwrap_or(u64::MAX),
                        )))
                        .build()
                        .map_err(anyhow::Error::from)?,
                ),
            };
            let items = query_builder
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
                let last = result
                    .last()
                    .ok_or(anyhow!("Expects at least one entry in the vector"))?;
                Some(match order {
                    Order::Asc => Cursor::FromEntriesQuery {
                        start_date: last.created_at,
                        end_date: *end_date,
                        order: order.clone(),
                        account_id: account_id.clone(),
                        sequence: last.sequence,
                    },
                    Order::Desc => Cursor::FromEntriesQuery {
                        start_date: *start_date,
                        end_date: last.created_at,
                        order: order.clone(),
                        account_id: account_id.clone(),
                        sequence: last.sequence,
                    },
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
            .key("pk", Pk::Balance(account_id.clone()).into())
            .key("sk", Sk::CurrentEntry.into())
            .send()
            .await
            .map_err(anyhow::Error::from)?
            .item()
            .map(|item| -> Result<(HashMap<LedgerBalanceName, i128>, u64)> {
                Ok((
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
                        .collect::<Result<HashMap<LedgerBalanceName, i128>>>()?,
                    item.get("sequence")
                        .ok_or(anyhow!(
                            "Missing sequence for HEAD of account_id {}",
                            account_id.to_string()
                        ))?
                        .as_n()
                        .map_err(|_| anyhow!("Not a number"))?
                        .parse()
                        .map_err(|err| anyhow!("Error parsing sequence number: {err}"))?,
                ))
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
                    sequence: entry_with_balance.sequence + 1,
                    created_at: utc_now(),
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
                                .and_then(|(balances, _)| {
                                    balances.get(&ledger_balance_name).cloned()
                                })
                                .unwrap_or(0);
                            let new_balance = balance + value;
                            (ledger_balance_name, new_balance)
                        })
                        .collect(),
                    status: entry.status.clone(),
                    ledger_fields: entry.ledger_fields.clone(),
                    additional_fields: entry.additional_fields.clone(),
                    sequence: head_balances
                        .as_ref()
                        .map(|(_, sequence)| sequence + 1)
                        .unwrap_or(0),
                    created_at: utc_now(),
                },
            };
            entries_with_balance.push(new_entry);
        }
        for entry in entries_with_balance.iter() {
            transact = transact.transact_items(create_transact_item_for_entry(entry, false)?);
        }
        match head_balances {
            Some((balance, last_sequence)) => {
                let entry = entries_with_balance.last().ok_or(anyhow!(
                    "Missing last entry for account_id {}",
                    account_id.to_string()
                ))?;
                transact = transact.transact_items(
                    TransactWriteItem::builder()
                        .update(
                            Update::builder()
                                .table_name("a_ledger")
                                .key("pk", Pk::Balance(account_id.clone()).into())
                                .key("sk", Sk::CurrentEntry.into())
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
                                    ":sequence",
                                    AttributeValue::N(
                                        entry.sequence.to_string(),
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
                                .expression_attribute_values(
                                    ":old_sequence",
                                    AttributeValue::N(
                                        last_sequence.to_string(),
                                    ),
                                )
                                .expression_attribute_names("#sequence_field", "sequence")
                                .update_expression("SET ledger_balances = :ledger_balances, ledger_fields = :ledger_fields, additional_fields = :additional_fields, entry_id = :entry_id, created_at = :created_at, entry_status = :status, #sequence_field = :sequence")
                                .condition_expression("ledger_balances = :old_ledger_balances AND #sequence_field = :old_sequence")
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
    let (pk, sk) = match (is_head, &entry.status) {
        (true, _) => (Pk::Balance(entry.account_id.clone()), Sk::CurrentEntry),
        (false, EntryStatus::Reverted(sequence)) => (
            Pk::Entry(entry.account_id.clone(), entry.entry_id.clone()),
            Sk::RevertedEntry(*sequence),
        ),
        (false, EntryStatus::Revert(_)) => (
            Pk::Entry(entry.account_id.clone(), entry.entry_id.clone()),
            Sk::RevertEntry,
        ),
        (false, EntryStatus::Applied) => (
            Pk::Entry(entry.account_id.clone(), entry.entry_id.clone()),
            Sk::CurrentEntry,
        ),
    };
    let mut put_builder = Put::builder()
        .table_name("a_ledger")
        .item("pk", pk.into())
        .item("sk", sk.into())
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
            if is_head {
                AttributeValue::S("head".into())
            } else {
                AttributeValue::S(format!(
                    "{}|{}",
                    entry.account_id,
                    entry.created_at.date_naive()
                ))
            },
        )
        .item(
            "entry_status",
            AttributeValue::S(serde_json::to_string(&entry.status)?),
        )
        .item("sequence", AttributeValue::N(entry.sequence.to_string()))
        .item(
            "created_at",
            AttributeValue::S(format_created_at_and_sequence(
                &entry.created_at,
                entry.sequence,
            )),
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
    let pk = Pk::try_from(
        item.get("pk")
            .ok_or(GetBalanceError::MissingField("pk".into()))?
            .clone(),
    )?;
    let (account_id, entry_id) = match pk {
        Pk::Entry(account_id, entry_id) => (account_id, entry_id),
        Pk::Balance(account_id) => (
            account_id,
            EntryId::new_unchecked(
                item.get("entry_id")
                    .ok_or(GetBalanceError::MissingField("entry_id".into()))?
                    .as_s()
                    .map_err(|_| GetBalanceError::ErrorReadingField("entry_id".into()))?
                    .clone(),
            ),
        ),
    };

    let mut created_at = item
        .get("created_at")
        .ok_or(GetBalanceError::MissingField("created_at".into()))?
        .as_s()
        .map_err(|_| GetBalanceError::ErrorReadingField("created_at".into()))?
        .as_str();
    if let Some((separated_created_at, _sequence)) = created_at.split_once('|') {
        created_at = separated_created_at;
    }
    Ok(EntryWithBalance {
        account_id,
        entry_id,
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
        sequence: item
            .get("sequence")
            .ok_or(GetBalanceError::MissingField("sequence".into()))?
            .as_n()
            .map_err(|_| GetBalanceError::ErrorReadingField("sequence".into()))?
            .parse::<u64>()
            .map_err(|_| GetBalanceError::ErrorReadingField("sequence".into()))?,
        created_at: DateTime::from_str(created_at)
            .map_err(|_| GetBalanceError::ErrorReadingField("created_at".into()))?,
    })
}

enum Pk {
    Entry(AccountId, EntryId),
    Balance(AccountId),
}

impl From<Pk> for AttributeValue {
    fn from(value: Pk) -> Self {
        match value {
            Pk::Entry(account_id, entry_id) => {
                AttributeValue::S(format!("ACCOUNT_ID:{}|ENTRY_ID:{}", account_id, entry_id))
            }
            Pk::Balance(account_id) => AttributeValue::S(format!("ACCOUNT_ID:{}", account_id)),
        }
    }
}

impl TryFrom<AttributeValue> for Pk {
    type Error = anyhow::Error;

    fn try_from(value: AttributeValue) -> Result<Self, Self::Error> {
        let value = value
            .as_s()
            .map_err(|_| anyhow!("Expect PK to be a string"))?;
        if let Some((account, entry)) = value.split_once('|') {
            let Some(account_id) = account.strip_prefix("ACCOUNT_ID:") else {
                bail!("Expected ACCOUNT_ID: prefix")
            };
            let Some(entry_id) = entry.strip_prefix("ENTRY_ID:") else {
                bail!("Expected ENTRY_ID: prefix")
            };
            return Ok(Pk::Entry(
                AccountId::new(Uuid::from_str(account_id)?),
                EntryId::new_unchecked(entry_id.into()),
            ));
        }
        let Some(account_id) = value.strip_prefix("ACCOUNT_ID:") else {
            bail!("Expected ACCOUNT_ID: prefix")
        };
        Ok(Pk::Balance(AccountId::new(Uuid::from_str(account_id)?)))
    }
}

enum Sk {
    CurrentEntry,
    RevertEntry,
    RevertedEntry(u64),
}

impl From<Sk> for AttributeValue {
    fn from(value: Sk) -> Self {
        match value {
            Sk::CurrentEntry => AttributeValue::S("|~".into()),
            Sk::RevertEntry => AttributeValue::S("|REVERT".into()),
            Sk::RevertedEntry(sequence) => {
                AttributeValue::S(format!("|REVERT_ENTRY_SEQUENCE:{}", sequence))
            }
        }
    }
}

impl TryFrom<AttributeValue> for Sk {
    type Error = anyhow::Error;

    fn try_from(value: AttributeValue) -> Result<Self, Self::Error> {
        let value = value
            .as_s()
            .map_err(|_| anyhow!("Expect PK to be a string"))?;
        if value == "|~" {
            return Ok(Sk::CurrentEntry);
        }
        if value == "|REVERT" {
            return Ok(Sk::RevertEntry);
        }
        let Some(sequence) = value.strip_prefix("|REVERT_ENTRY_SEQUENCE:") else {
            bail!("Expected REVERT_ENTRY_ID: prefix")
        };
        Ok(Sk::RevertedEntry(sequence.parse()?))
    }
}

#[cfg(test)]
pub mod test {
    use tokio::sync::Mutex;

    use super::*;

    pub struct LedgerEntryRepositoryForTests {
        internal_state: Mutex<InternalState>,
    }

    struct InternalState {
        append_entries_call_count: u32,
        append_entries_response: Vec<Result<Vec<EntryWithBalance>, AppendEntriesError>>,
    }

    impl LedgerEntryRepositoryForTests {
        pub fn new() -> Self {
            Self {
                internal_state: Mutex::new(InternalState {
                    append_entries_call_count: 0,
                    append_entries_response: Vec::new(),
                }),
            }
        }

        pub async fn push_append_entries_response(
            &self,
            response: Result<Vec<EntryWithBalance>, AppendEntriesError>,
        ) {
            let mut internal_state = self.internal_state.lock().await;
            internal_state.append_entries_response.push(response)
        }

        pub async fn get_append_entries_call_count(&self) -> u32 {
            self.internal_state.lock().await.append_entries_call_count
        }
    }

    impl LedgerEntryRepository for LedgerEntryRepositoryForTests {
        async fn append_entries(
            &self,
            _account_id: &AccountId,
            _entries: &[Entry],
        ) -> Result<Vec<EntryWithBalance>, AppendEntriesError> {
            let mut internal_state = self.internal_state.lock().await;
            internal_state.append_entries_call_count += 1;
            internal_state.append_entries_response.remove(0)
        }

        async fn revert_entries(
            &self,
            _account_id: &AccountId,
            _entries: &[EntryId],
        ) -> Result<Vec<EntryWithBalance>, RevertEntriesError> {
            todo!()
        }

        async fn get_balance(
            &self,
            _account_id: &AccountId,
        ) -> Result<EntryWithBalance, GetBalanceError> {
            todo!()
        }

        async fn get_entry(
            &self,
            _account_id: &AccountId,
            _entry_id: &EntryId,
            _entry_to_continue: EntryToContinue,
            _limit: u8,
        ) -> Result<Vec<EntryWithBalance>, GetBalanceError> {
            todo!()
        }

        async fn get_entries(
            &self,
            _account_id: &AccountId,
            _start_date: &DateTime<Utc>,
            _end_date: &DateTime<Utc>,
            _limit: u8,
            _order: &Order,
            _sequence: Option<u64>,
        ) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
            todo!()
        }
    }
}

fn format_created_at_and_sequence(created_at: &DateTime<Utc>, sequence: u64) -> String {
    format!("{}|{:0>20}", created_at, sequence)
}
