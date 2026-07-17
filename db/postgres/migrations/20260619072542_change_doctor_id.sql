DROP TABLE IF EXISTS doctor_consultation_config CASCADE;
DROP TABLE IF EXISTS doctor_fee_transaction CASCADE;
DROP TABLE IF EXISTS doctor_clinic CASCADE;

CREATE TABLE IF NOT EXISTS doctor_consultation_config
(
    doctor_id           uuid                                                                       not null,
    supported_languages language_code_enum[]     default '{th,en}'::language_code_enum[]           not null,
    channel_types       channel_type_enum[]      default '{voice,chat,video}'::channel_type_enum[] not null,
    duration_minutes    int4,
    doctor_fee_amount   numeric(10, 2),
    created_at          timestamp with time zone default now()                                     not null,
    updated_at          timestamp with time zone,
    CONSTRAINT doctor_configuration_pkey PRIMARY KEY (doctor_id),
    CONSTRAINT doctor_duration_minutes_check CHECK (duration_minutes IS NULL OR duration_minutes = ANY (ARRAY [15,30,50])),
    CONSTRAINT doctor_fee_amount_positive_check CHECK (doctor_fee_amount IS NULL OR doctor_fee_amount >= 0)
);

CREATE TABLE IF NOT EXISTS doctor_fee_transaction
(
    transaction_id      integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    doctor_id           uuid                     not null,
    doctor_fee_amount   numeric(10, 2)           not null,
    previous_fee_amount numeric(10, 2),
    change_reason       text,
    action_by           integer                  not null,
    created_at          timestamp with time zone not null default now(),
    CONSTRAINT doctor_fee_transaction_amount_positive_check CHECK (doctor_fee_amount >= 0)
);
CREATE INDEX IF NOT EXISTS idx_doctor_fee_transaction_account_created ON doctor_fee_transaction (doctor_id, created_at DESC);

CREATE TABLE IF NOT EXISTS doctor_clinic
(
    doctor_id  uuid    not null,
    clinic_id  integer not null,
    created_at timestamp with time zone default now(),
    updated_at timestamp with time zone,
    CONSTRAINT doctor_clinic_pkey PRIMARY KEY (doctor_id, clinic_id)
);
CREATE INDEX IF NOT EXISTS idx_doctor_clinic_clinic ON doctor_clinic (clinic_id);
CREATE INDEX IF NOT EXISTS idx_doctor_clinic_doctor_id ON doctor_clinic (doctor_id);




