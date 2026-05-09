-- +goose Up
CREATE TABLE meetings (
    meeting_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    meeting_name text NOT NULL CHECK (char_length(meeting_name) > 0),
    meeting_date date NOT NULL DEFAULT CURRENT_DATE,
    created_at timestamptz NOT NULL DEFAULT now(),
    active bool NOT NULL DEFAULT false,
    voteit_token text NOT NULL,
    kerberos_token uuid UNIQUE NOT NULL DEFAULT gen_random_uuid()
);

CREATE TABLE attendances (
    meeting_id uuid NOT NULL REFERENCES meetings (meeting_id) ON DELETE CASCADE,
    email text NOT NULL,
    entered_at timestamptz NOT NULL DEFAULT now(),
    entered_at_item text NULL,
    left_at timestamptz NULL,
    left_at_item text NULL,
    suffrage bool NOT NULL DEFAULT false,
    PRIMARY KEY (meeting_id, email, entered_at),
    CONSTRAINT check_dates CHECK (left_at IS NULL OR (left_at >= entered_at))
);

CREATE UNIQUE INDEX idx_one_active_attendance_per_meeting
    ON attendances (meeting_id, email)
    WHERE (left_at IS NULL);

-- +goose Down
DROP TABLE attendances;
DROP TABLE meetings;
