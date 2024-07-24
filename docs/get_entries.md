# Get Entries

This endpoint is used to get all entries of an account. This is triggered by receiving a GET request in the endpoint `api/v1/{:account_id}/entry`. 
It will return the entries of this account with a cursor to get the next page of entries.

There are some query params that you need to provide and some that are optional. Here is the list of query params:

- **limit**: The number of entries that you want to get. This is required, and it should be a number between 1 and 255.
- **start_date** (Optional): The start_date to query for entries. You need to provide either a start_date and end_date or a cursor.
- **end_date** (Optional): The end_date to query for entries.
- **order** (Optional): The order of the entries. It can be `asc` or `desc`. Default is desc.
- **cursor** (Optional): The cursor to get the next page of entries.

Here is an example of request and response:

```
GET http://127.0.0.1:3001/api/v1/balance/f5700a39-8f31-4a1f-8bd5-3b35ccc61568/entry?limit=3&start_date=2024-07-22T00%3A00%3A00Z&end_date=2024-07-23T15%3A58%3A18.404Z
```

```
HTTP/1.1 200 OK

{
  "entries": [
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5871",
      "ledger_balances": {
        "balance_local_amount": 761608,
        "balance_usd_amount": 1378147
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
      "status": "Applied",
      "created_at": "2024-07-22T19:32:09.582500Z"
    },
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5867",
      "ledger_balances": {
        "balance_local_amount": 711363,
        "balance_usd_amount": 1278003
      },
      "ledger_fields": {
        "usd_amount": 604001,
        "local_amount": 340341
      },
      "additional_fields": {
        "description": "Transfer",
        "fx_rate": 5.01,
        "local_currency": "BRL"
      },
      "status": "Applied",
      "created_at": "2024-07-22T19:31:49.164158Z"
    },
    {
      "account_id": "f5700a39-8f31-4a1f-8bd5-3b35ccc61568",
      "entry_id": "d5348939-d402-4deb-a0d1-eba6199b5865",
      "ledger_balances": {
        "balance_usd_amount": 674002,
        "balance_local_amount": 371022
      },
      "ledger_fields": {
        "usd_amount": 604001,
        "local_amount": 340341
      },
      "additional_fields": {
        "description": "Transfer",
        "fx_rate": 5.01,
        "local_currency": "BRL"
      },
      "status": "Applied",
      "created_at": "2024-07-22T19:31:43.468676Z"
    }
  ],
  "cursor": "eyJGcm9tRW50cmllc1F1ZXJ5Ijp7ImFjY291bnRfaWQiOiJmNTcwMGEzOS04ZjMxLTRhMWYtOGJkNS0zYjM1Y2NjNjE1NjgiLCJzdGFydF9kYXRlIjoiMjAyNC0wNy0yMlQwMDowMDowMFoiLCJlbmRfZGF0ZSI6IjIwMjQtMDctMjJUMTk6MzE6NDMuNDY4Njc2WiIsInNlcXVlbmNlIjozLCJvcmRlciI6IkRlc2MifX0="
}
```