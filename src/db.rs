use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, postgres::PgQueryResult, types::Uuid};

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

pub struct Meeting {
    pub meeting_id: Uuid,
    pub meeting_name: String,
    pub meeting_date: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub active: bool,
    pub voteit_token: String,
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

async fn create_meeting(
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

async fn remove_meeting(db: &Db, id: Uuid) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!("DELETE FROM meetings WHERE meeting_id = $1", id)
        .execute(db.pool())
        .await
}

async fn get_meeting(db: &Db, id: Uuid) -> Result<Meeting, sqlx::Error> {
    sqlx::query_as!(Meeting, "SELECT * FROM meetings WHERE meeting_id=$1", id)
        .fetch_one(db.pool())
        .await
}

async fn is_meeting_active(db: &Db, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!("SELECT active FROM meetings WHERE meeting_id=$1", id)
        .fetch_one(db.pool())
        .await
}

/*
func UpdateAttendance(db *sqlx.DB, meetingID uuid.UUID, email string, ts time.Time, suffrage bool) error {
    isActive, err := isMeetingActive(db, meetingID)
    if err != nil {
        return fmt.Errorf("meeting %s does not exist", meetingID)
    }
    if !isActive {
        return fmt.Errorf("meeting %s is inactive", meetingID)
    }

    query := `
        WITH updated AS (
            UPDATE attendances
            SET left_at = $1
            WHERE meeting_id = $2 AND email = $3 AND left_at IS NULL
            RETURNING 1
        )
        INSERT INTO attendances (meeting_id, email, entered_at, suffrage)
        SELECT $2, $3, $1, $4
        WHERE NOT EXISTS (SELECT 1 FROM updated);
    `

    _, err = db.Exec(query, ts, meetingID, email, suffrage)
    if err != nil {
        return fmt.Errorf("failed to toggle attendance for %s: %w", email, err)
    }

    return nil
}
 * */
