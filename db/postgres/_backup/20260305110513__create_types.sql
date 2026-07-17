-- sqlfluff:dialect:postgres

-- Custom ENUM types for doctor onboarding system

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
