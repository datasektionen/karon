package database

import (
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
)

type Meeting struct {
	MeetingID     uuid.UUID `db:"meeting_id"`
	MeetingName   string    `db:"meeting_name"`
	MeetingDate   time.Time `db:"meeting_date"`
	CreatedAt     time.Time `db:"created_at"`
	Active        bool      `db:"active"`
	VoteITToken   string    `db:"voteit_token"`
	KerberosToken uuid.UUID `db:"kerberos_token"`
}

func CreateMeeting(db *sqlx.DB, name string, date time.Time, voteitToken string) (uuid.UUID, error) {
	var newID uuid.UUID

	query := `
        INSERT INTO meetings (meeting_name, meeting_date, voteit_token)
        VALUES ($1, $2, $3)
        RETURNING meeting_id
	`

	err := db.QueryRow(query, name, voteitToken, date).Scan(&newID)
	if err != nil {
		return uuid.Nil, err
	}

	return newID, nil
}

func RemoveMeeting(db *sqlx.DB, id uuid.UUID) error {
	query := `DELETE FROM meetings WHERE meeting_id = $1`

	_, err := db.Exec(query, id)
	if err != nil {
		return fmt.Errorf("failed to delete meeting %s: %w", id, err)
	}

	return nil
}

func GetMeeting(db *sqlx.DB, id uuid.UUID) (Meeting, error) {
	meeting := Meeting{}
	err := db.Get(&meeting, `SELECT * FROM meetings WHERE meeting_id=$1`, id)
	return meeting, err
}

func isMeetingActive(db *sqlx.DB, id uuid.UUID) (bool, error) {
	active := false
	err := db.Get(&active, `SELECT active FROM meetings WHERE meeting_id=$1`, id)
	return active, err
}

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
