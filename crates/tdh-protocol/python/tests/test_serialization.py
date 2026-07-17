import pytest
from tdh_protocol.common import (
    ConsultationEvent, TimeslotReserved, PatientIdentity, ConsultationChannel
)


class TestConsultationEvent:
    """Test ConsultationEvent serialization and deserialization."""

    def test_timeslot_reserved_roundtrip(self):
        """Test TimeslotReserved variant serialization."""
        event = ConsultationEvent()
        event.timeslot_reserved.booking_id = "booking-123"
        event.timeslot_reserved.doctor_id = 2443
        event.timeslot_reserved.biz_unit_id = 1
        event.timeslot_reserved.reserved_from = 1677648000
        event.timeslot_reserved.reservation_duration_sec = 1800
        event.timeslot_reserved.consultation_channel = ConsultationChannel.CONSULTATION_CHANNEL_VIDEO
        event.timeslot_reserved.reserved_at = 1677648000

        # Set patient identity
        event.timeslot_reserved.patient_identity.account_id = 123
        event.timeslot_reserved.patient_identity.user_profile_id = 456
        event.timeslot_reserved.patient_identity.tenant_id = 789

        # Serialize to bytes
        bytes_data = event.SerializeToString()

        # Deserialize
        decoded = ConsultationEvent()
        decoded.ParseFromString(bytes_data)

        # Assert
        assert decoded.timeslot_reserved.booking_id == "booking-123"
        assert decoded.timeslot_reserved.doctor_id == 2443


class TestPatientIdentity:
    """Test PatientIdentity serialization."""

    def test_with_optional_field(self):
        """Test PatientIdentity with optional oidc_user_id."""
        patient = PatientIdentity()
        patient.account_id = 123
        patient.user_profile_id = 456
        patient.tenant_id = 789
        patient.oidc_user_id = "auth-0"

        bytes_data = patient.SerializeToString()

        decoded = PatientIdentity()
        decoded.ParseFromString(bytes_data)

        assert decoded.account_id == 123
        assert decoded.oidc_user_id == "auth-0"


class TestEnums:
    """Test enum serialization."""

    def test_consultation_channel(self):
        """Test ConsultationChannel enum values."""
        channels = [
            ConsultationChannel.CONSULTATION_CHANNEL_VIDEO,
            ConsultationChannel.CONSULTATION_CHANNEL_CHAT,
            ConsultationChannel.CONSULTATION_CHANNEL_VOICE,
        ]

        for channel in channels:
            # Verify enum values are in expected range
            assert channel >= 1 and channel <= 3
