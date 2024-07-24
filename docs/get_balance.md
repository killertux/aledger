# Get Balance

This endpoint is used to get the balance of an account. This is triggered by receiving a GET request in the endpoint `api/v1/balance/{:account_id}`. 

Here is an example of request and response:

```
GET http://127.0.0.1:3001/api/v1/balance/f5700a39-8f31-4a1f-8bd5-3b35ccc61568
```

```
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
```

If the account does not exist, a 404 status will be returned.
