import {
  ConsultationEvent,
  TimeslotReserved,
  PatientIdentity,
  ConsultationChannel,
} from '../src/index';

describe('ConsultationEvent', () => {
  describe('serialization', () => {
    test('should serialize and deserialize TimeslotReserved', () => {
      const original: ConsultationEvent = {
        event: {
          case: 'timeslotReserved',
          value: {
            bookingId: 'booking-123',
            patientIdentity: {
              accountId: 123,
              userProfileId: 456,
              tenantId: 789,
              oidcUserId: '',
            },
            doctorId: 2443,
            bizUnitId: 1,
            reservedFrom: BigInt(1677648000),
            reservationDurationSec: 1800,
            consultationChannel: ConsultationChannel.CONSULTATION_CHANNEL_VIDEO,
            reservedAt: BigInt(1677648000),
          },
        },
      };

      // Serialize to binary
      const bytes = ConsultationEvent.toBinary(original);

      // Deserialize
      const decoded = ConsultationEvent.fromBinary(bytes);

      // Assert
      expect(decoded.event.case).toBe('timeslotReserved');
      expect(decoded.event.value.bookingId).toBe('booking-123');
      expect(decoded.event.value.doctorId).toBe(2443);
    });
  });
});

describe('PatientIdentity', () => {
  test('should serialize with optional fields', () => {
    const withOptional: PatientIdentity = {
      accountId: 123,
      userProfileId: 456,
      tenantId: 789,
      oidcUserId: 'auth-0',
    };

    const bytes = PatientIdentity.toBinary(withOptional);
    const decoded = PatientIdentity.fromBinary(bytes);

    expect(decoded.oidcUserId).toBe('auth-0');
  });
});
