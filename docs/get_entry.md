# Get Entry

This endpoint is used to get a specific entry of an account. This is triggered by receiving a GET request in the endpoint `api/v1/balance/{:account_id}/entry/{:entry_id}`. This will return  all events associated to an entry. A entry can have multiple events if it was reversed.
