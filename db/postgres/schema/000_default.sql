-- sqlfluff:dialect:postgres

CREATE TYPE language_code_enum AS ENUM ('th', 'en');
CREATE TYPE workplace_type_enum AS ENUM ('primary', 'additional');
CREATE TYPE specialty_type_enum AS ENUM ('specialty', 'sub_specialty');
CREATE TYPE specialty_category_enum AS ENUM ('specialty', 'sub_specialty');
CREATE TYPE channel_type_enum AS ENUM ('voice', 'chat', 'video');
CREATE TYPE specialty_level_enum AS ENUM ('primary', 'additional');
CREATE TYPE approval_status_enum AS ENUM ('approved', 'rejected', 'pending');
CREATE TYPE document_type_enum AS ENUM (
    'profile_image',
    'id_card_image',
    'book_bank_image',
    'med_license_image',
    'certificate_image'
    );

CREATE TYPE "onboarding_status_enum" AS ENUM (
    'Draft',
    'PendingApproval',
    'CancelledByUser',
    'Approved',
    'Rejected',
    'Deactivated'
    );

-- Main onboarding table
CREATE TABLE IF NOT EXISTS "onboarding"
(
    "doctor_account_id"        integer                  NOT NULL PRIMARY KEY,
    "citizen_id"               character varying(13)    NOT NULL,
    "profession_id"            integer                  NOT NULL,
    "academic_position_id"     integer                  NOT NULL,
    "license_number"           character varying(50)    NOT NULL,
    "medical_school"           character varying(255)   NOT NULL,
    "status"                   "onboarding_status_enum" NOT NULL DEFAULT 'Draft',
    "status_reason"            text                     NULL,                  -- For Rejected/Deactivated statuses
    "address_detail"           text                     NOT NULL,
    "sub_district_id"          integer                  NOT NULL,
    "district_id"              integer                  NOT NULL,
    "province_id"              integer                  NOT NULL,
    "postal_code_id"           integer                  NOT NULL,
    "profile_image_url"        character varying(500)   NOT NULL,
    "id_card_image_url"        character varying(500)   NOT NULL,
    "book_bank_image_url"      character varying(500)   NOT NULL,
    "med_license_image_url"    character varying(500)   NOT NULL,
    "certificate_image_urls"   text[]                   NOT NULL DEFAULT '{}',
    "special_interests"        text[]                   NOT NULL DEFAULT '{}',
    "name_en_firstname"        character varying(100)   NOT NULL,
    "name_en_lastname"         character varying(100)   NOT NULL,
    "name_th_firstname"        character varying(100)   NOT NULL,
    "name_th_lastname"         character varying(100)   NOT NULL,
    "primary_workplace_ids"    integer[]                NOT NULL,
    "additional_workplace_ids" integer[]                NOT NULL DEFAULT '{}',
    "specialties"              jsonb                    NOT NULL DEFAULT '[]', -- Array of specialty objects
    "created_at"               timestamptz              NOT NULL DEFAULT now(),
    "updated_at"               timestamptz              NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS "idx_onboarding_status" ON "onboarding" ("status");
CREATE INDEX IF NOT EXISTS "idx_onboarding_created_at" ON "onboarding" (
                                                                        "created_at"
    );


CREATE TABLE IF NOT EXISTS doctor
(
    doctor_id                 uuid PRIMARY KEY,
    citizen_id                varchar(13)          NOT NULL UNIQUE,
    profession_id             integer              NOT NULL,
    academic_position_id      integer,
    department_id             integer,
    primary_medical_school_id integer,
    license_number            varchar(50)          NOT NULL UNIQUE,
    special_interest          text[], -- ???
    profile_image_url         varchar(500)         NOT NULL,
    id_card_image_url         varchar(500)         NOT NULL,
    book_bank_image_url       varchar(500)         NOT NULL,
    med_license_image_url     varchar(500)         NOT NULL,
    supported_languages       language_code_enum[]          DEFAULT '{th}',
    approval_status           approval_status_enum NOT NULL,
    is_active                 boolean              NOT NULL DEFAULT FALSE,
    created_at                timestamp with time zone      DEFAULT now(),
    updated_at                timestamp with time zone,
    CONSTRAINT chk_citizen_id_length CHECK (length(citizen_id) = 13)
);

CREATE TABLE IF NOT EXISTS doctor_availability
(
    doctor_id             uuid PRIMARY KEY,
    instant_mode_enabled  boolean     DEFAULT FALSE,
    schedule_mode_enabled boolean     DEFAULT FALSE,
    updated_at            timestamptz DEFAULT now()
);

CREATE TABLE IF NOT EXISTS doctor_case
(
    doctor_id   uuid PRIMARY KEY,
    case_amount integer     DEFAULT 0,
    updated_at  timestamptz DEFAULT now()
);

CREATE TABLE IF NOT EXISTS department
(
    department_id    serial PRIMARY KEY,
    name             jsonb NOT NULL,
    counseling_areas jsonb,
    created_at       timestamp with time zone DEFAULT now(),
    updated_at       timestamp with time zone
);

CREATE TABLE IF NOT EXISTS doctor_name_i18n
(
    doctor_id  uuid PRIMARY KEY,
    firstname  jsonb NOT NULL,
    lastname   jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now()
);

CREATE TABLE IF NOT EXISTS doctor_address
(
    doctor_id       uuid PRIMARY KEY,
    address_detail  text,
    sub_district_id integer,
    district_id     integer,
    province_id     integer,
    created_at      timestamp with time zone DEFAULT now(),
    updated_at      timestamp with time zone
);

CREATE TABLE IF NOT EXISTS doctor_workplace
(
    doctor_id                uuid PRIMARY KEY,
    primary_workplace_id     integer NOT NULL,
    additional_workplace_ids integer[],
    created_at               timestamp with time zone DEFAULT now(),
    updated_at               timestamp with time zone
);

CREATE TABLE IF NOT EXISTS doctor_specialty
(
    doctor_specialty_id serial PRIMARY KEY,
    doctor_id           uuid                 NOT NULL,
    specialty_id        integer              NOT NULL,
    medical_school_id   integer              NOT NULL,
    specialty_level     specialty_level_enum NOT NULL,
    created_at          timestamptz DEFAULT now(),
    UNIQUE (doctor_id, specialty_id, medical_school_id)
);

CREATE TABLE IF NOT EXISTS doctor_sub_specialty
(
    doctor_specialty_id serial PRIMARY KEY
        REFERENCES doctor_specialty (doctor_specialty_id) ON DELETE CASCADE,
    sub_specialty_id    integer NOT NULL,
    medical_school_id   integer NOT NULL,
    created_at          timestamptz DEFAULT now()
);

CREATE TABLE IF NOT EXISTS doctor_certificate_document
(
    document_id serial PRIMARY KEY,
    doctor_id   uuid   NOT NULL,
    url         text[] NOT NULL,
    created_at  timestamp with time zone DEFAULT now(),
    deleted_at  timestamp with time zone
);

CREATE TABLE IF NOT EXISTS doctor_channel
(
    doctor_id    uuid              NOT NULL,
    channel_type channel_type_enum NOT NULL,
    is_enabled   boolean                  DEFAULT TRUE,
    created_at   timestamp with time zone DEFAULT now(),
    updated_at   timestamp with time zone,
    PRIMARY KEY (doctor_id, channel_type)
);

CREATE TABLE IF NOT EXISTS doctor_fee
(
    doctor_fee_id serial PRIMARY KEY,
    doctor_id     uuid                                   NOT NULL,
    fee_amount    decimal(10, 2)                         NOT NULL,
    currency      varchar(3)               DEFAULT 'THB' NOT NULL,
    created_at    timestamp with time zone DEFAULT now(),
    deleted_at    timestamp with time zone,
    CONSTRAINT chk_fee_amount_positive CHECK (fee_amount >= 0)
);

create type doctor_profile_status_enum as enum ('Draft','PendingApproval', 'Approved', 'Rejected', 'Deactivated');

CREATE TABLE IF NOT EXISTS doctor_profile_draft
(
    doctor_account_id           integer                    not null primary key,
    doctor_profile_id           integer                    not null,
    citizen_id                  text,
    profession                  jsonb                    default '[]'::jsonb,
    academic_position           jsonb                    default '[]'::jsonb,
    first_name                  jsonb                    default '[]'::jsonb,
    last_name                   jsonb                    default '[]'::jsonb,
    license_number              varchar(50),
    primary_medical_school      jsonb                    default '[]'::jsonb,
    specialty                   jsonb                    default '{}'::jsonb,
    additional_specialties      jsonb                    default '[]'::jsonb,
    special_interest            text[]                   default '{}'::text[],
    address_detail              text,
    sub_district                jsonb,
    district                    jsonb,
    province                    jsonb,
    postal_code                 integer,
    work_place                  jsonb                    default '[]'::jsonb,
    additional_workplace        jsonb                    default '[]'::jsonb,
    profile_image_url           varchar(500),
    id_card_image_url           varchar(500),
    book_bank_image_url         varchar(500),
    medical_license_image_url   varchar(500),
    education_license_image_url text[]                   default '{}'::text[],
    status                      doctor_profile_status_enum not null,
    created_at                  timestamp with time zone default now(),
    updated_at                  timestamp with time zone
);
CREATE INDEX idx_doctor_profile_draft_doctor_account_id ON doctor_profile_draft (doctor_account_id);

CREATE TABLE IF NOT EXISTS doctor_profile
(
    doctor_id                   uuid                                          not null,
    doctor_account_id           integer                                       not null,
    doctor_profile_id           integer                                       not null,
    citizen_id                  text                                          not null,
    profession                  jsonb                    default '[]'::jsonb  not null,
    academic_position           jsonb                    default '[]'::jsonb  not null,
    first_name                  jsonb                    default '[]'::jsonb  not null,
    last_name                   jsonb                    default '[]'::jsonb  not null,
    department_id               integer                                       not null,
    license_number              varchar(50)                                   not null,
    primary_medical_school      jsonb                    default '[]'::jsonb  not null,
    specialty                   jsonb                    default '{}'::jsonb  not null,
    additional_specialties      jsonb                    default '[]'::jsonb  not null,
    special_interest            text[]                   default '{}'::text[] not null,
    address_detail              text                                          not null,
    sub_district                jsonb                                         not null,
    district                    jsonb                                         not null,
    province                    jsonb                                         not null,
    postal_code                 integer                                       not null,
    work_place                  jsonb                    default '[]'::jsonb  not null,
    additional_workplace        jsonb                    default '[]'::jsonb  not null,
    profile_image_url           varchar(500)                                  not null,
    id_card_image_url           varchar(500)                                  not null,
    book_bank_image_url         varchar(500)                                  not null,
    medical_license_image_url   varchar(500)                                  not null,
    education_license_image_url text[]                   default '{}'::text[] not null,
    is_active                   boolean                  default false        not null,
    profile_version             bigint                   default 0            not null,
    created_at                  timestamp with time zone default now(),
    updated_at                  timestamp with time zone,
    CONSTRAINT doctor_profile_pkey PRIMARY KEY (doctor_id),
    CONSTRAINT doctor_profile_account_key UNIQUE (doctor_account_id)
);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_doctor_account_id ON doctor_profile (doctor_account_id);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_specialty ON doctor_profile USING gin (specialty jsonb_path_ops);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_department_id ON doctor_profile (department_id);

-- Transactional integration-event outbox. The relay owns leases and delivery
-- bookkeeping; producers write a complete immutable payload with the profile mutation.
CREATE TABLE IF NOT EXISTS doctor_profile_event_outbox
(
    event_id           uuid PRIMARY KEY,
    doctor_id          uuid                     NOT NULL REFERENCES doctor_profile (doctor_id),
    doctor_account_id  integer                  NOT NULL,
    event_type         text                     NOT NULL,
    schema_version     integer                  NOT NULL,
    profile_version    bigint                   NOT NULL,
    occurred_at        timestamptz              NOT NULL,
    payload            jsonb                    NOT NULL,
    attempts           integer                  NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at       timestamptz              NOT NULL DEFAULT now(),
    lease_token        uuid,
    leased_until       timestamptz,
    published_at       timestamptz,
    last_error         text,
    created_at         timestamptz              NOT NULL DEFAULT now(),
    CONSTRAINT doctor_profile_event_outbox_profile_version_key UNIQUE (doctor_id, profile_version)
);
CREATE INDEX IF NOT EXISTS idx_doctor_profile_event_outbox_ready
    ON doctor_profile_event_outbox (available_at)
    WHERE published_at IS NULL;

CREATE TABLE IF NOT EXISTS doctor_rating
(
    doctor_id  uuid PRIMARY KEY REFERENCES doctor_profile (doctor_id) ON DELETE CASCADE,
    rating     numeric(3, 1) NOT NULL DEFAULT 0 CHECK (rating >= 0 AND rating <= 5),
    updated_at timestamptz   NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS doctor_score
(
    doctor_id  uuid PRIMARY KEY REFERENCES doctor_profile (doctor_id) ON DELETE CASCADE,
    score      integer     NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_doctor_score_ranking ON doctor_score (score DESC, doctor_id DESC);

CREATE TABLE IF NOT EXISTS doctor_profile_transaction
(
    transaction_id    integer generated always as identity primary key,
    doctor_account_id integer                    not null,
    status            doctor_profile_status_enum not null,
    status_reason     text                       not null,
    action_by         varchar(50),
    created_at        timestamp with time zone default now()
);
CREATE INDEX idx_doctor_profile_transaction_doctor_account_id ON doctor_profile_transaction (doctor_account_id);

CREATE TABLE IF NOT EXISTS clinics
(
    clinic_id  integer not null,
    name       jsonb   not null,
    created_at timestamp with time zone default now(),
    updated_at timestamp with time zone,
    CONSTRAINT clinic_pkey PRIMARY KEY (clinic_id)
);
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
    CONSTRAINT doctor_duration_minutes_check CHECK (duration_minutes IS NULL OR duration_minutes = ANY (ARRAY [15,25,50])),
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
