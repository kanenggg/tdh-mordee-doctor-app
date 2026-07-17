DROP TABLE IF EXISTS doctor_profile CASCADE;
CREATE TABLE doctor_profile
(
    doctor_id                   uuid                                          NOT NULL,
    doctor_account_id           integer                                       NOT NULL,
    doctor_profile_id           integer                                       NOT NULL,
    citizen_id                  text                                          NOT NULL,
    profession                  jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    academic_position           jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    first_name                  jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    last_name                   jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    department_id               integer                                       NOT NULL,
    license_number              varchar(50)                                   NOT NULL,
    primary_medical_school      jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    specialty                   jsonb                    DEFAULT '{}'::jsonb  NOT NULL,
    additional_specialties      jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    special_interest            text[]                   DEFAULT '{}'::text[] NOT NULL,
    address_detail              text                                          NOT NULL,
    sub_district                jsonb                                         NOT NULL,
    district                    jsonb                                         NOT NULL,
    province                    jsonb                                         NOT NULL,
    postal_code                 integer                                       NOT NULL,
    work_place                  jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    additional_workplace        jsonb                    DEFAULT '[]'::jsonb  NOT NULL,
    profile_image_url           varchar(500)                                  NOT NULL,
    id_card_image_url           varchar(500)                                  NOT NULL,
    book_bank_image_url         varchar(500)                                  NOT NULL,
    medical_license_image_url   varchar(500)                                  NOT NULL,
    education_license_image_url text[]                   DEFAULT '{}'::text[] NOT NULL,
    is_active                   boolean                  DEFAULT false        NOT NULL,
    created_at                  timestamp with time zone DEFAULT now(),
    updated_at                  timestamp with time zone,
    CONSTRAINT doctor_profile_pkey PRIMARY KEY (doctor_id),
    CONSTRAINT doctor_profile_account_key UNIQUE (doctor_account_id)
);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_doctor_account_id ON doctor_profile (doctor_account_id);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_specialty ON doctor_profile USING gin (specialty jsonb_path_ops);


CREATE TABLE doctor_consultation_config
(
    doctor_account_id   integer                                                                    NOT NULL,
    supported_languages language_code_enum[]     DEFAULT '{th,en}'::language_code_enum[]           NOT NULL,
    channel_types       channel_type_enum[]      DEFAULT '{voice,chat,video}'::channel_type_enum[] NOT NULL,
    duration_minutes    int4,
    doctor_fee_amount   numeric(10, 2),
    created_at          timestamp with time zone DEFAULT now()                                     NOT NULL,
    updated_at          timestamp with time zone,
    CONSTRAINT doctor_configuration_pkey PRIMARY KEY (doctor_account_id),
    CONSTRAINT doctor_duration_minutes_check CHECK (duration_minutes IS NULL OR duration_minutes = ANY (ARRAY [15, 30, 50])),
    CONSTRAINT doctor_fee_amount_positive_check CHECK (doctor_fee_amount IS NULL OR doctor_fee_amount >= 0)
);

CREATE TABLE doctor_fee_transaction
(
    transaction_id      integer GENERATED ALWAYS AS IDENTITY,
    doctor_account_id   integer                  NOT NULL,
    doctor_fee_amount   numeric(10, 2)           NOT NULL,
    previous_fee_amount numeric(10, 2),
    change_reason       text,
    action_by           integer                  NOT NULL,
    created_at          timestamp with time zone NOT NULL DEFAULT now(),
    CONSTRAINT doctor_fee_transaction_pkey PRIMARY KEY (transaction_id),
    CONSTRAINT doctor_fee_transaction_amount_positive_check CHECK (doctor_fee_amount >= 0)
);
CREATE INDEX IF NOT EXISTS idx_doctor_fee_transaction_account_created ON doctor_fee_transaction (doctor_account_id, created_at DESC);

CREATE TABLE clinics
(
    clinic_id  integer NOT NULL,
    name       jsonb   NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone,
    CONSTRAINT clinic_pkey PRIMARY KEY (clinic_id)
);

CREATE TABLE doctor_clinic
(
    doctor_account_id integer NOT NULL,
    clinic_id         integer NOT NULL,
    created_at        timestamp with time zone DEFAULT now(),
    updated_at        timestamp with time zone,
    CONSTRAINT doctor_clinic_pkey PRIMARY KEY (doctor_account_id, clinic_id)
);
CREATE INDEX IF NOT EXISTS idx_doctor_clinic_clinic ON doctor_clinic (clinic_id);
CREATE INDEX IF NOT EXISTS idx_doctor_clinic_doctor_id ON doctor_clinic (doctor_account_id);
