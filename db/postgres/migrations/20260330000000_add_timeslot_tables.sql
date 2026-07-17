-- Create enum type "timeslot_status_enum"
CREATE TYPE "timeslot_status_enum" AS ENUM ('Free', 'Reserved', 'Confirmed');

-- Create "timeslots" table
CREATE TABLE "timeslots" (
    "id" character varying(64) NOT NULL,
    "doctor_id" integer NOT NULL,
    "start_time" bigint NOT NULL,
    "end_time" bigint NOT NULL,
    "is_instant" boolean NOT NULL DEFAULT false,
    "status" "timeslot_status_enum" NOT NULL DEFAULT 'Free',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);

-- Create enum type "reservation_status_enum"
CREATE TYPE "reservation_status_enum" AS ENUM ('Pending', 'Confirmed', 'Cancelled', 'Expired');

-- Create "reservations" table
CREATE TABLE "reservations" (
    "id" character varying(64) NOT NULL,
    "timeslot_id" character varying(64) NOT NULL,
    "doctor_id" integer NOT NULL,
    "patient_id" integer NOT NULL,
    "status" "reservation_status_enum" NOT NULL DEFAULT 'Pending',
    "correlation_id" character varying(128) NOT NULL,
    "booking_id" character varying(64) NULL,
    "payment_reference" character varying(128) NULL,
    "expires_at" bigint NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "confirmed_at" timestamptz NULL,
    "cancelled_at" timestamptz NULL,
    PRIMARY KEY ("id"),
    CONSTRAINT "reservations_timeslot_id_fkey" FOREIGN KEY ("timeslot_id") REFERENCES "timeslots" ("id") ON UPDATE NO ACTION ON DELETE RESTRICT,
    CONSTRAINT "reservations_correlation_id_key" UNIQUE ("correlation_id")
);

-- Create "rate_limit_counts" table for PostgreSQL-based rate limiting
CREATE TABLE "rate_limit_counts" (
    "id" serial NOT NULL,
    "patient_id" integer NOT NULL,
    "limit_type" character varying(16) NOT NULL,
    "window_start" bigint NOT NULL,
    "count" integer NOT NULL DEFAULT 1,
    PRIMARY KEY ("id"),
    CONSTRAINT "rate_limit_counts_patient_id_limit_type_window_start_key" UNIQUE ("patient_id", "limit_type", "window_start"),
    CONSTRAINT "chk_limit_type" CHECK (limit_type IN ('daily', 'weekly'))
);

-- Create index "idx_timeslots_doctor_status" to table: "timeslots"
CREATE INDEX "idx_timeslots_doctor_status" ON "timeslots" ("doctor_id", "status");

-- Create index "idx_timeslots_doctor_time" to table: "timeslots"
CREATE INDEX "idx_timeslots_doctor_time" ON "timeslots" ("doctor_id", "start_time", "end_time");

-- Create index "idx_timeslots_status_time" to table: "timeslots"
CREATE INDEX "idx_timeslots_status_time" ON "timeslots" ("status", "start_time");

-- Create index "idx_reservations_expires" to table: "reservations"
CREATE INDEX "idx_reservations_expires" ON "reservations" ("expires_at") WHERE status = 'Pending';

-- Create index "idx_reservations_patient" to table: "reservations"
CREATE INDEX "idx_reservations_patient" ON "reservations" ("patient_id");

-- Create index "idx_reservations_status" to table: "reservations"
CREATE INDEX "idx_reservations_status" ON "reservations" ("status");

-- Create index "idx_ratelimit_patient_window" to table: "rate_limit_counts"
CREATE INDEX "idx_ratelimit_patient_window" ON "rate_limit_counts" ("patient_id", "limit_type", "window_start");

-- Create index "idx_ratelimit_expires" to table: "rate_limit_counts"
CREATE INDEX "idx_ratelimit_expires" ON "rate_limit_counts" ("window_start");

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_timeslots_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to automatically update updated_at
CREATE TRIGGER "update_timeslots_updated_at"
    BEFORE UPDATE ON "timeslots"
    FOR EACH ROW
    EXECUTE FUNCTION update_timeslots_updated_at_column();
