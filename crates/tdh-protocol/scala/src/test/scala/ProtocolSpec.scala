import org.scalatest.wordspec.AnyWordSpec
import org.scalatest.matchers.should.Matchers
import tdh.protocol.common._

class ProtocolSpec extends AnyWordSpec with Matchers {

  "ConsultationEvent" should {

    "serialize and deserialize TimeslotReserved" in {
      val original = ConsultationEvent(
        event = ConsultationEvent.Event.TimeslotReserved(
          TimeslotReserved(
            bookingId = "booking-123",
            patientIdentity = Some(
              PatientIdentity(
                accountId = 123,
                userProfileId = 456,
                tenantId = 789,
                oidcUserId = ""
              )
            ),
            doctorId = 2443,
            bizUnitId = 1,
            reservedFrom = 1677648000L,
            reservationDurationSec = 1800,
            consultationChannel = ConsultationChannel.VIDEO,
            reservedAt = 1677648000L
          )
        )
      )

      val bytes = original.toByteArray
      val decoded = ConsultationEvent.parseFrom(bytes)

      decoded shouldBe original
    }

    "handle all ConsultationChannel enum values" in {
      val channels = Seq(
        ConsultationChannel.VIDEO,
        ConsultationChannel.CHAT,
        ConsultationChannel.VOICE
      )

      for (channel <- channels) {
        channel.value should be >= 1
        channel.value should be <= 3
      }
    }
  }

  "PatientIdentity" should {

    "serialize with optional fields" in {
      val patient = PatientIdentity(
        accountId = 123,
        userProfileId = 456,
        tenantId = 789,
        oidcUserId = "auth-0"
      )

      val bytes = patient.toByteArray
      val decoded = PatientIdentity.parseFrom(bytes)

      decoded.oidcUserId shouldBe "auth-0"
    }
  }
}
