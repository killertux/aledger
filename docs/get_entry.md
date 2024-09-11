# Get Entry

This endpoint is used to get a specific entry of an account. This is triggered by receiving a GET request in the endpoint `api/v1/balance/{:account_id}/entry/{:entry_id}`. This will return  all events associated to an entry. An entry can have multiple events if it was reversed.

There are some query params that you need to provide and some that are optional. Here is the list of query params:

- **limit**: The number of entries that you want to get. This is required, and it should be a number between 1 and 255.
- **cursor** (Optional): The cursor to get the next page of entries.

Here is an example of request and response:

```
GET http://127.0.0.1:3001/api/v1/balance/f5700a39-8f31-4a1f-8bd5-3b35ccc61568/entry/d5348939-d402-4deb-a0d1-eba6199b5872?limit=10
```

```
{
  "entries": [
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5872",
      "ledger_balances": {
        "balance_usd_amount": 0,
        "balance_local_amount": 0
      },
      "ledger_fields": {
        "local_amount": -50245,
        "usd_amount": -100144
      },
      "additional_fields": {
        "description": "Transfer",
        "fx_rate": 5.01,
        "local_currency": "BRL"
      },
      "status": "Revert",
      "created_at": "2024-09-11T16:43:05.184916Z"
    },
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5872",
      "ledger_balances": {
        "balance_usd_amount": 100144,
        "balance_local_amount": 50245
      },
      "ledger_fields": {
        "local_amount": 50245,
        "usd_amount": 100144
      },
      "additional_fields": {
        "description": "Transfer",
        "fx_rate": 5.01,
        "local_currency": "BRL"
      },
      "status": "Reverted",
      "created_at": "2024-09-11T16:42:42.258553Z"
    }
  ]
}
```