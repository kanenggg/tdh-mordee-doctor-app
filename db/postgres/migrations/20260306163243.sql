-- Create enum type "approval_status_enum"
CREATE TYPE "approval_status_enum" AS ENUM ('approved', 'rejected', 'pending');
-- Create enum type "language_code_enum"
CREATE TYPE "language_code_enum" AS ENUM ('th', 'en');
-- Create "doctor" table
CREATE TABLE "doctor" (
    "doctor_id" uuid NOT NULL,
    "citizen_id" character varying(13) NOT NULL,
    "profession_id" integer NOT NULL,
    "academic_position_id" integer NULL,
    "department_id" integer NULL,
    "primary_medical_school_id" integer NULL,
    "license_number" character varying(50) NOT NULL,
    "special_interest" text [] NULL,
    "profile_image_url" character varying(500) NOT NULL,
    "id_card_image_url" character varying(500) NOT NULL,
    "book_bank_image_url" character varying(500) NOT NULL,
    "med_license_image_url" character varying(500) NOT NULL,
    "supported_languages" "language_code_enum" [] NULL DEFAULT '{th}',
    "approval_status" "approval_status_enum" NOT NULL,
    "is_active" boolean NOT NULL DEFAULT false,
    "created_at" timestamptz NULL DEFAULT now(),
    "updated_at" timestamptz NULL,
    PRIMARY KEY ("doctor_id"),
    CONSTRAINT "doctor_citizen_id_key" UNIQUE ("citizen_id"),
    CONSTRAINT "doctor_license_number_key" UNIQUE ("license_number"),
    CONSTRAINT "chk_citizen_id_length" CHECK (length((citizen_id)::text) = 13)
);
-- Create "doctor_address" table
CREATE TABLE "doctor_address" (
    "doctor_id" uuid NOT NULL,
    "address_detail" text NULL,
    "sub_district_id" integer NULL,
    "district_id" integer NULL,
    "province_id" integer NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    "updated_at" timestamptz NULL,
    PRIMARY KEY ("doctor_id")
);
-- Create enum type "specialty_type_enum"
CREATE TYPE "specialty_type_enum" AS ENUM ('specialty', 'sub_specialty');
-- Create enum type "specialty_category_enum"
CREATE TYPE "specialty_category_enum" AS ENUM ('specialty', 'sub_specialty');
-- Create enum type "channel_type_enum"
CREATE TYPE "channel_type_enum" AS ENUM ('voice', 'chat', 'video');
-- Create enum type "specialty_level_enum"
CREATE TYPE "specialty_level_enum" AS ENUM ('primary', 'additional');
-- Create enum type "document_type_enum"
CREATE TYPE "document_type_enum" AS ENUM (
    'profile_image',
    'id_card_image',
    'book_bank_image',
    'med_license_image',
    'certificate_image'
);
-- Create enum type "onboarding_status_enum"
CREATE TYPE "onboarding_status_enum" AS ENUM (
    'Draft',
    'PendingApproval',
    'CancelledByUser',
    'Approved',
    'Rejected',
    'Deactivated'
);
-- Create "doctor_availability" table
CREATE TABLE "doctor_availability" (
    "doctor_id" uuid NOT NULL,
    "instant_mode_enabled" boolean NULL DEFAULT false,
    "schedule_mode_enabled" boolean NULL DEFAULT false,
    "updated_at" timestamptz NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id")
);
-- Create enum type "workplace_type_enum"
CREATE TYPE "workplace_type_enum" AS ENUM ('primary', 'additional');
-- Create "department" table
CREATE TABLE "department" (
    "department_id" serial NOT NULL,
    "name" jsonb NOT NULL,
    "counseling_areas" jsonb NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    "updated_at" timestamptz NULL,
    PRIMARY KEY ("department_id")
);
-- Create "doctor_case" table
CREATE TABLE "doctor_case" (
    "doctor_id" uuid NOT NULL,
    "case_amount" integer NULL DEFAULT 0,
    "updated_at" timestamptz NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id")
);
-- Create "doctor_certificate_document" table
CREATE TABLE "doctor_certificate_document" (
    "document_id" serial NOT NULL,
    "doctor_id" uuid NOT NULL,
    "url" text [] NOT NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    "deleted_at" timestamptz NULL,
    PRIMARY KEY ("document_id")
);
-- Create "doctor_channel" table
CREATE TABLE "doctor_channel" (
    "doctor_id" uuid NOT NULL,
    "channel_type" "channel_type_enum" NOT NULL,
    "is_enabled" boolean NULL DEFAULT true,
    "created_at" timestamptz NULL DEFAULT now(),
    "updated_at" timestamptz NULL,
    PRIMARY KEY ("doctor_id", "channel_type")
);
-- Create "doctor_fee" table
CREATE TABLE "doctor_fee" (
    "doctor_fee_id" serial NOT NULL,
    "doctor_id" uuid NOT NULL,
    "fee_amount" numeric(10, 2) NOT NULL,
    "currency" character varying(3) NOT NULL DEFAULT 'THB',
    "created_at" timestamptz NULL DEFAULT now(),
    "deleted_at" timestamptz NULL,
    PRIMARY KEY ("doctor_fee_id"),
    CONSTRAINT "chk_fee_amount_positive" CHECK (fee_amount >= (0)::numeric)
);
-- Create "doctor_name_i18n" table
CREATE TABLE "doctor_name_i18n" (
    "doctor_id" uuid NOT NULL,
    "firstname" jsonb NOT NULL,
    "lastname" jsonb NOT NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    PRIMARY KEY ("doctor_id")
);
-- Create "doctor_specialty" table
CREATE TABLE "doctor_specialty" (
    "doctor_specialty_id" serial NOT NULL,
    "doctor_id" uuid NOT NULL,
    "specialty_id" integer NOT NULL,
    "medical_school_id" integer NOT NULL,
    "specialty_level" "specialty_level_enum" NOT NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    PRIMARY KEY ("doctor_specialty_id"),
    CONSTRAINT "doctor_specialty_doctor_id_specialty_id_medical_school_id_key" UNIQUE (
        "doctor_id", "specialty_id", "medical_school_id"
    )
);
-- Create "onboarding" table
CREATE TABLE "onboarding" (
    "doctor_account_id" integer NOT NULL,
    "citizen_id" character varying(13) NOT NULL,
    "profession_id" integer NOT NULL,
    "academic_position_id" integer NOT NULL,
    "license_number" character varying(50) NOT NULL,
    "medical_school" character varying(255) NOT NULL,
    "status" "onboarding_status_enum" NOT NULL DEFAULT 'Draft',
    "status_reason" text NULL,
    "address_detail" text NOT NULL,
    "sub_district_id" integer NOT NULL,
    "district_id" integer NOT NULL,
    "province_id" integer NOT NULL,
    "postal_code_id" integer NOT NULL,
    "profile_image_url" character varying(500) NOT NULL,
    "id_card_image_url" character varying(500) NOT NULL,
    "book_bank_image_url" character varying(500) NOT NULL,
    "med_license_image_url" character varying(500) NOT NULL,
    "certificate_image_urls" text [] NOT NULL DEFAULT '{}',
    "special_interests" text [] NOT NULL DEFAULT '{}',
    "name_en_firstname" character varying(100) NOT NULL,
    "name_en_lastname" character varying(100) NOT NULL,
    "name_th_firstname" character varying(100) NOT NULL,
    "name_th_lastname" character varying(100) NOT NULL,
    "primary_workplace_ids" integer [] NOT NULL,
    "additional_workplace_ids" integer [] NOT NULL DEFAULT '{}',
    "specialties" jsonb NOT NULL DEFAULT '[]',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("doctor_account_id")
);
-- Create index "idx_onboarding_created_at" to table: "onboarding"
CREATE INDEX "idx_onboarding_created_at" ON "onboarding" ("created_at");
-- Create index "idx_onboarding_status" to table: "onboarding"
CREATE INDEX "idx_onboarding_status" ON "onboarding" ("status");
-- Create "doctor_workplace" table
CREATE TABLE "doctor_workplace" (
    "doctor_id" uuid NOT NULL,
    "primary_workplace_id" integer NOT NULL,
    "additional_workplace_ids" integer [] NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    "updated_at" timestamptz NULL,
    PRIMARY KEY ("doctor_id")
);
-- Create "doctor_sub_specialty" table
CREATE TABLE "doctor_sub_specialty" (
    "doctor_specialty_id" serial NOT NULL,
    "sub_specialty_id" integer NOT NULL,
    "medical_school_id" integer NOT NULL,
    "created_at" timestamptz NULL DEFAULT now(),
    PRIMARY KEY ("doctor_specialty_id"),
    CONSTRAINT "doctor_sub_specialty_doctor_specialty_id_fkey" FOREIGN KEY (
        "doctor_specialty_id"
    ) REFERENCES "doctor_specialty" (
        "doctor_specialty_id"
    ) ON UPDATE NO ACTION ON DELETE CASCADE
);
