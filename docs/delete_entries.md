# Delete Entries.

This endpoint is used to delete entries from an account. The way that this works is that a new event will be created to compensate the original event. After a deletion, you can re-send an entry with the same entry_id again that it will be accepted.

Here is an example of request and response:

```
DELETE http://127.0.0.1:3001/api/v1/balance
Content-Type: application/json

[
  {
    "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
    "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5872"
  }
]
```

```
{
  "applied_entries": [
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
    }
  ],
  "non_applied_entries": []
}
```