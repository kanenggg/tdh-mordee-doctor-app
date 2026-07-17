-- Doctor rating (follows doctor_case pattern)
CREATE TABLE IF NOT EXISTS "doctor_rating" (
    "doctor_id" uuid NOT NULL,
    "rating" numeric(3,1) NOT NULL DEFAULT 0
        CHECK (rating >= 0 AND rating <= 5),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id"),
    CONSTRAINT "fk_doctor_rating_doctor" FOREIGN KEY ("doctor_id") REFERENCES "doctor" ("doctor_id") ON DELETE CASCADE
);

-- Doctor ranking score
-- score = integer score used for sorting (higher = better, any value allowed)
-- ranked/i_ranked are computed at runtime from Redis sorted set positions, NOT stored in DB
CREATE TABLE IF NOT EXISTS "doctor_score" (
    "doctor_id" uuid NOT NULL,
    "score" integer NOT NULL DEFAULT 0,
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id"),
    CONSTRAINT "fk_doctor_score_doctor" FOREIGN KEY ("doctor_id") REFERENCES "doctor" ("doctor_id") ON DELETE CASCADE
);

-- Composite index for cursor pagination (score DESC, doctor_id DESC)
CREATE INDEX IF NOT EXISTS "idx_doctor_score_ranking" ON "doctor_score" ("score" DESC, "doctor_id" DESC);

-- Doctor consultation duration (supports multiple durations per doctor)
CREATE TABLE IF NOT EXISTS "doctor_duration" (
    "doctor_id" uuid NOT NULL,
    "duration_minutes" integer NOT NULL
        CHECK (duration_minutes IN (15, 30, 50)),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id"),
    CONSTRAINT "uq_doctor_duration" UNIQUE ("doctor_id", "duration_minutes"),
    CONSTRAINT "fk_doctor_duration_doctor" FOREIGN KEY ("doctor_id") REFERENCES "doctor" ("doctor_id") ON DELETE CASCADE
);
