CREATE TABLE IF NOT EXISTS meetings (
    meeting_id integer PRIMARY KEY,
    meeting_name text NOT NULL,
    meeting_date date NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    active bool NOT NULL DEFAULT false,
);

CREATE TABLE IF NOT EXISTS attendences (
    meeting_id text NOT NULL REFERENCES meetings (meeting_id) ON DELETE CASCADE,
    email text NOT NULL,
    entered_at timestamptz NOT NULL DEFAULT now(),
    left_at timestamptz,
    suffrage bool NOT NULL,
    PRIMARY KEY (meeting_id, email, entered_at)
);
