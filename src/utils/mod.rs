use chrono::{DateTime, Utc};

#[cfg(not(test))]
pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
pub fn utc_now() -> DateTime<Utc> {
    test::now()
}

#[cfg(test)]
pub mod test {
    use std::cell::Cell;

    use chrono::{DateTime, Utc};

    thread_local! {
        static TIMESTAMP: Cell<i64> = const { Cell::new(0) };
    }

    pub fn now() -> DateTime<Utc> {
        TIMESTAMP.with(|timestamp| {
            if timestamp.get() == 0 {
                timestamp.set(Utc::now().timestamp())
            }
            DateTime::from_timestamp(timestamp.get(), 0).expect("a valid timestamp set")
        })
    }

    pub fn set_now(now: &DateTime<Utc>) {
        TIMESTAMP.set(now.timestamp());
    }
}
