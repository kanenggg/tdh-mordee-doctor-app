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

CREATE TABLE doctor (
                        doctor_id                   UUID PRIMARY KEY,
                        citizen_id                  VARCHAR(13) NOT NULL UNIQUE,
                        profession_id               INTEGER NOT NULL,
                        academic_position_id        INTEGER,
                        department_id               INTEGER,
                        primary_medical_school_id   INTEGER,
                        license_number              VARCHAR(50) NOT NULL UNIQUE,
                        special_interest            TEXT[], -- ???
                        profile_image_url           VARCHAR(500) NOT NULL,
                        id_card_image_url           VARCHAR(500) NOT NULL,
                        book_bank_image_url         VARCHAR(500) NOT NULL,
                        med_license_image_url       VARCHAR(500) NOT NULL,
                        supported_languages         language_code_enum[] DEFAULT '{th}',
                        approval_status             approval_status_enum NOT NULL,
                        is_active                   BOOLEAN NOT NULL DEFAULT FALSE,
                        created_at                  TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                        updated_at                  TIMESTAMP WITH TIME ZONE,
                        CONSTRAINT chk_citizen_id_length CHECK (LENGTH(citizen_id) = 13)
);

CREATE TABLE doctor_availability (
                                     doctor_id              UUID PRIMARY KEY,
                                     instant_mode_enabled   BOOLEAN DEFAULT FALSE,
                                     schedule_mode_enabled  BOOLEAN DEFAULT FALSE,
                                     updated_at             TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE doctor_case (
                             doctor_id    UUID PRIMARY KEY,
                             case_amount  INTEGER DEFAULT 0,
                             updated_at   TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE department (
                            department_id           SERIAL PRIMARY KEY,
                            name                    JSONB NOT NULL,
                            counseling_areas        JSONB,
                            created_at              TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                            updated_at              TIMESTAMP WITH TIME ZONE
);

CREATE TABLE doctor_name_i18n (
                                  doctor_id       UUID PRIMARY KEY,
                                  firstname       JSONB NOT NULL,
                                  lastname        JSONB NOT NULL,
                                  created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE doctor_address (
                                doctor_id       UUID PRIMARY KEY,
                                address_detail  TEXT,
                                sub_district_id INTEGER,
                                district_id     INTEGER,
                                province_id     INTEGER,
                                created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                                updated_at      TIMESTAMP WITH TIME ZONE
);

CREATE TABLE doctor_workplace (
                                  doctor_id                   UUID PRIMARY KEY,
                                  primary_workplace_id        INTEGER NOT NULL,
                                  additional_workplace_ids    INTEGER[],
                                  created_at                  TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                                  updated_at                  TIMESTAMP WITH TIME ZONE
);

CREATE TABLE doctor_specialty (
                                  doctor_specialty_id SERIAL PRIMARY KEY,
                                  doctor_id           UUID NOT NULL,
                                  specialty_id        INTEGER NOT NULL,
                                  medical_school_id   INTEGER NOT NULL,
                                  specialty_level     specialty_level_enum NOT NULL,
                                  created_at          TIMESTAMPTZ DEFAULT NOW(),
                                  UNIQUE (doctor_id, specialty_id, medical_school_id)
);

CREATE TABLE doctor_sub_specialty (
                                      doctor_specialty_id SERIAL PRIMARY KEY
                                          REFERENCES doctor_specialty(doctor_specialty_id) ON DELETE CASCADE,
                                      sub_specialty_id    INTEGER NOT NULL,
                                      medical_school_id   INTEGER NOT NULL,
                                      created_at          TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE doctor_certificate_document (
                                             document_id         SERIAL PRIMARY KEY,
                                             doctor_id           UUID NOT NULL,
                                             url                 TEXT[] NOT NULL,
                                             created_at          TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                                             deleted_at          TIMESTAMP WITH TIME ZONE
);

CREATE TABLE doctor_channel (
                                doctor_id       UUID NOT NULL,
                                channel_type    channel_type_enum NOT NULL,
                                is_enabled      BOOLEAN DEFAULT true,
                                created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                                updated_at      TIMESTAMP WITH TIME ZONE,
                                PRIMARY KEY (doctor_id, channel_type)
);

CREATE TABLE doctor_fee (
                            doctor_fee_id   SERIAL PRIMARY KEY,
                            doctor_id       UUID NOT NULL,
                            fee_amount      DECIMAL(10, 2) NOT NULL,
                            currency        VARCHAR(3) DEFAULT 'THB' NOT NULL,
                            created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                            deleted_at      TIMESTAMP WITH TIME ZONE,
                            CONSTRAINT chk_fee_amount_positive CHECK (fee_amount >= 0)
);

