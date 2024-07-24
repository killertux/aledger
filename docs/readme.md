# A Ledger Documentation

This is A Ledger implementation using rust and DynamoDB. It was built considering the following requirements:

- The system should be able to handle a large number of transactions.
- The system should be able to handle a large number of accounts.
- The system should guarantee uniqueness of events. (An event cannot be processed more than once).
- The system should allow event reversals without affecting the history.
- The system should allow queries to check the balance of an account at a given point in time.

## Architecture

DynamoDB is really the star here. A Ledger is basically a simple layer over it that maps how to use the PK/SK and GSIs.
Because of this, the architecture is really basically how we map our data into DynamoDb.

### PK
We have two types od PKs, the first one represents an event in an account that we call **ENTRY** PK, and the second one represents the current balance of the account that we call **BALANCE** PK.

The Entry PK is composed of the account id and the event id using the following structure:
```
ACCOUNT_ID:{account_id}|ENTRY_ID:{entry_id}
```

The Balance PK is composed of the account id using the following structure:
```
ACCOUNT_ID:{account_id}
```

Every new entry in an account will cause a new insert in the table with the **ENTRY** PK and a new update in the table with the **BALANCE** PK. We use a optimistic lock approach in the **BALANCE** PK to guarantee that we are not updating the balance with an outdated value and protect against concurrency errors.

### SK

We also have two types of SKs, the first one represents the current entry **CurrentEntry** and the second one represents a **History**.

The CurrentEntry SK is always represent by the string `|~`. The history SK is composed as fo
```
|HISTORY:{sequence}
```
The sequence is a number that represents the order of the event in the account.

For PKs of the type **Balance**, we only use the CurrentEntry SK.

For PKs of the type **Entry**, we use the CurrentEntry SK for the current entry and the History SK for the history. The history is created to handle reversals. More details about it when we talk about the event reversal.

### GSIs

We use just one GSI that is only used the query historic data of the account. If you don't need this feature, you can remove it.

The GSI PK is composed of the account id and the date of the event using the following structure:
```
{account_id}|{date}
```
Where the date is in the format `YYYY-MM-DD`.

The GSI SK is composed of the created_at of the event.

The reason that we use the date in the PK is to avoid having a partition with too many items (This could reach the 10GB limit of DynamoDB). So, we are creating a new partition every day.

## Event Uniqueness and Reversals

Whenever a new entry is created, we try to insert a new row with the PK of type **ENTRY** and the SK of type **CurrentEntry**. If the row is already there, we return an error to the user and do not change the account balance. This way we guarantee uniqueness of events per account.

But, sometimes we might need to revert an event, so we can push it again with a different value. Imagine that due ot a bug, we created a bunch of events with the wrong value. To handle this, we created the reversal feature.

To revert an event, we create a new event with the oposite amount of the original one. This reverts the impact on the account balance. But, due to the uniqueness constrain, this alone will not allow you to re-insert the event with the correct value. To handle this, we delete the **CurrentEntry** row of the original event and create a new one with the **History** SK.
We use the sequence of the event in the history so we can easily check the history in sequence of all changes to a specific entry in the balance (imagine a event that was reverted and re-inserted multiple times).

## Endpoint Docs

You can find more detailed endpoint docs here:

- [Push Entries](./push_entries.md)
- [Get Balance](./get_balance.md)
- [Get Entries](./get_entries.md)
- [Get Entry](./get_entry.md)
- [Delete Entries](./delete_entries.md)
