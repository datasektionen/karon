//! Functions related directly to the database.

use chrono::{DateTime, Days, NaiveDate, Utc};
use sqlx::{PgPool, postgres::PgQueryResult, types::Uuid};

use crate::{server::Error, voteit::VoteItToken};

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

/// Representation of a chapter meeting in the database.
pub struct Meeting {
    pub meeting_id: Uuid,
    pub meeting_name: String,
    pub meeting_date: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub active: bool,
    pub voteit_token: VoteItToken,
    pub kerberos_token: Uuid,
}

impl Db {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Creates a meeting with a VoteIT token linked to a certain VoteIT meeting. The token does not
/// have to be unique. The meeting will by default not be active. This function returns the internal
/// id of the meeting.
pub async fn create_meeting(
    db: &Db,
    name: String,
    date: NaiveDate,
    voteit_token: String,
) -> Result<Uuid, sqlx::Error> {
    let id = sqlx::query_scalar!(
        r"
        INSERT INTO meetings (meeting_name, meeting_date, voteit_token)
        VALUES ($1, $2, $3)
        RETURNING meeting_id
        ",
        name,
        date,
        voteit_token,
    )
    .fetch_one(db.pool())
    .await?;

    Ok(id)
}

/// Enables a meeting, setting it to active.
pub async fn enable_meeting(db: &Db, id: Uuid) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        "UPDATE meetings SET active = true WHERE meeting_id = $1",
        id
    )
    .execute(db.pool())
    .await
}

/// Disables a meeting, changing it to inactive.
pub async fn disable_meeting(db: &Db, id: Uuid) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        "UPDATE meetings SET active = false WHERE meeting_id = $1",
        id
    )
    .execute(db.pool())
    .await
}

/// Removes a meeting from the database.
pub async fn remove_meeting(db: &Db, id: Uuid) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!("DELETE FROM meetings WHERE meeting_id = $1", id)
        .execute(db.pool())
        .await
}

/// Gets [`Meeting`] information based on a meeting id.
pub async fn get_meeting(db: &Db, id: Uuid) -> Result<Meeting, sqlx::Error> {
    sqlx::query_as!(Meeting, "SELECT * FROM meetings WHERE meeting_id=$1", id)
        .fetch_one(db.pool())
        .await
}

/// Gets [`Meeting`] information based on a `kerberos token`.
pub async fn get_meeting_from_token(db: &Db, token_str: &str) -> Result<Meeting, Error> {
    let token = Uuid::parse_str(token_str)?;
    sqlx::query_as!(
        Meeting,
        "SELECT * FROM meetings WHERE kerberos_token=$1",
        token
    )
    .fetch_one(db.pool())
    .await
    .map_err(|e| e.into())
}

/// Checks if a given meeting is active, based on the internal meeting id.
pub async fn is_meeting_active(db: &Db, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!("SELECT active FROM meetings WHERE meeting_id=$1", id)
        .fetch_one(db.pool())
        .await
}

/// The types of actions a member can take at a meeting.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Entered,
    Left,
}

/// Updates a members attendance at a given meeting, meaning they either [`Action::Left`] the
/// meeting if they were already present or [`Action::Entered`] if they were currently not in
/// attendance.
pub async fn update_attendance(
    db: &Db,
    meeting_id: Uuid,
    email: &str,
    timestamp: DateTime<Utc>,
    suffrage: bool,
) -> Result<Action, sqlx::Error> {
    let action = sqlx::query_scalar!(
        r#"
        WITH updated AS (
            UPDATE attendances
            SET left_at = $1
            WHERE meeting_id = $2 AND email = $3 AND left_at IS NULL
            RETURNING 'left' AS action
        ),
        inserted AS (
            INSERT INTO attendances (meeting_id, email, entered_at, suffrage)
            SELECT $2, $3, $1, $4
            WHERE NOT EXISTS (SELECT 1 FROM updated)
            RETURNING 'entered' AS action
        )
        SELECT action FROM updated
        UNION ALL
        SELECT action FROM inserted;
        "#,
        timestamp,
        meeting_id,
        email,
        suffrage
    )
    .fetch_one(db.pool())
    .await?;

    let action_str = match action {
        Some(a) => a,
        None => return Err(sqlx::Error::RowNotFound),
    };
    match action_str.as_str() {
        "left" => Ok(Action::Left),
        _ => Ok(Action::Entered),
    }
}

/// Verifies is a given `onboard token` is active, meaning it exists and was created less than 48
/// hours ago.
pub async fn verify_onboard_token(db: &Db, token_str: &str) -> Result<(), Error> {
    let token = Uuid::parse_str(token_str)?;
    let time = sqlx::query_scalar!(
        "SELECT created_at FROM onboard_tokens WHERE kerberos_token=$1",
        token
    )
    .fetch_one(db.pool())
    .await?;

    let current_time = Utc::now();
    if current_time
        > time
            .checked_add_days(Days::new(2))
            .expect("Could not add time")
    {
        return Err(Error::TokenExpired);
    }

    Ok(())
}
