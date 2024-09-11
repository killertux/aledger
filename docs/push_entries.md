# Push Entries.

This is the main endpoint to add new entries to accounts. It accepts a list of entries, and it returns what entries were successfully appended to the ledger.

If an account does not exist, it will be created.

## Request

Here is an example of a request and its response.

```
POST 127.0.0.1:3001/api/v1/balance
Content-Type: application/json

[
  {
    "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
    "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5862",
    "ledger_fields": {
      "local_amount": 10000,
      "usd_amount": 2000
    },
    "additional_fields": {
      "description": "Transfer",
      "local_currency": "BRL",
      "fx_rate": 5.00
    }
  }
]
```

```
HTTP/1.1 200 OK

{
  "applied_entries": [
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5862",
      "ledger_balances": {
        "balance_usd_amount": 2000,
        "balance_local_amount": 10000
      },
      "ledger_fields": {
        "local_amount": 10000,
        "usd_amount": 2000
      },
      "additional_fields": {
        "description": "Transfer",
        "fx_rate": 5.0,
        "local_currency": "BRL"
      },
      "status": "Applied",
      "created_at": "2024-07-22T18:36:06.039567Z"
    }
  ],
  "non_applied_entries": []
}
```

In the request you can pass a list of entries to be appended. Each entry has an `account_id`, `entry_id`, `ledger_fields`, and `additional_fields`. The account and entry ids are straight forward. The `ledger_fields` are the fields that will be used to calculate the balance of the account. The `additional_fields` are extra fields that you can use to store extra information about the entry.

In the response we will show what was applied and what was not. In the result we create a `status` field. We also return the `created_at` of the entry and the result of the `ledger_balances` which is the result of each balance of the account after the entry was applied.

Here is an example of a failed request response:

```
HTTP/1.1 200 OK

{
  "applied_entries": [],
  "non_applied_entries": [
    {
      "error": "Entry already exists for this account",
      "error_code": 200,
      "entry": {
        "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
        "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5862",
        "ledger_fields": {
          "usd_amount": 2000,
          "local_amount": 10000
        },
        "additional_fields": {
          "description": "Transfer",
          "fx_rate": 5.0,
          "local_currency": "BRL"
        }
      }
    }
  ]
}
```

We return which entries were not applied, and the reason why. In this case, the entry was already in the ledger. We also have an `erro_code` field that can be used to identify the error.

The complete list of error codes can be found [here](./errors.md)

## Important considerations

Even though there is no hard limit on the number of entries that can be sent in a single request, it is recommended to send a maximum of 100 entries per request.

Also, the order of the entries in the request might not be preserved. The system will group the orders by account_id and then apply them.

If there are failures, the system will still try to apply the other entries. If you want a transaction behaviour and consistent ordering than check the [transaction endpoint](./transaction.md).

## Conditions

You can define conditions to apply the entries. For now the only condition that can be passed is that one balance_field is greater or equal to a value after the entry being applied. Here is an example of a request with conditions:

```
POST 127.0.0.1:3001/api/v1/balance
Content-Type: application/json

[
  {
    "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
    "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5877",
    "ledger_fields": {
      "local_amount": -1,
      "usd_amount": -1
    },
    "additional_fields": {
      "description": "Transfer",
      "local_currency": "BRL",
      "fx_rate": 5.01
    },
    "conditionals": [
      {
        "greater_than_or_equal_to": {
          "balance": "balance_usd_amount",
          "value": 0
       }
      }
    ]
  }
]
```
