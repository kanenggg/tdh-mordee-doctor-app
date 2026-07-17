Feature: Timeslot

  Scenario: Get available timeslots - successful query
    Given a "doctorId", "startTime", "endTime", and "UserIdentity"
    When Patient calls "GET /timeslot/v1/available"
    Then Patient should get a "GetAvailableTimeslotsResponse.AvailableTimeslots" response with timeslot list

  Scenario: Reserve timeslot - successful reservation
    Given a "timeslotId", "reservationTtlSeconds", and "UserIdentity" with a free timeslot
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "ReserveTimeslotResponse.Success" response with reservationId and expiresAt
    And Timeslot status should be "Reserved"
    And Reservation should be created with status "Pending"
    And "TimeslotReserved" event should be published to "appointments" topic

  Scenario: Reserve timeslot - timeslot already reserved
    Given a "timeslotId" and "UserIdentity" with a reserved timeslot
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "ReserveTimeslotResponse.AlreadyReserved" response

  Scenario: Reserve timeslot - timeslot not found
    Given a "timeslotId" and "UserIdentity" with a non-existent timeslot
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "ReserveTimeslotResponse.NotFound" response

  Scenario: Reserve timeslot - daily rate limit exceeded
    Given a "patientId" and "UserIdentity" with 10 reservations today
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "ReserveTimeslotResponse.RateLimitExceeded" response with limitType="daily" and retryAfterSeconds

  Scenario: Reserve timeslot - weekly rate limit exceeded
    Given a "patientId" and "UserIdentity" with 50 reservations this week
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "ReserveTimeslotResponse.RateLimitExceeded" response with limitType="weekly" and retryAfterSeconds

  Scenario: Reserve timeslot - TTL too low
    Given a "timeslotId", "UserIdentity", and "reservationTtlSeconds" less than 60
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "400 Bad Request" response with validation error

  Scenario: Reserve timeslot - TTL too high
    Given a "timeslotId", "UserIdentity", and "reservationTtlSeconds" greater than 3600
    When Patient calls "POST /timeslot/v1/reserve"
    Then Patient should get a "400 Bad Request" response with validation error

  Scenario: Confirm booking - successful confirmation
    Given a "reservationId", "bookingId", "paymentReference", and "UserIdentity" with a Pending reservation
    When Patient calls "POST /timeslot/v1/confirm"
    Then Patient should get a "ConfirmBookingResponse.Success" response
    And Reservation status should be "Confirmed"
    And Timeslot status should be "Confirmed"
    And bookingId should be set on reservation
    And "TimeslotConfirmed" event should be published to "appointments" topic

  Scenario: Confirm booking - reservation not found
    Given a "reservationId", "bookingId", and "UserIdentity" with a non-existent reservation
    When Patient calls "POST /timeslot/v1/confirm"
    Then Patient should get a "ConfirmBookingResponse.NotFound" response

  Scenario: Confirm booking - reservation already confirmed
    Given a "reservationId", "bookingId", and "UserIdentity" with a Confirmed reservation
    When Patient calls "POST /timeslot/v1/confirm"
    Then Patient should get a "400 Bad Request" response with "Reservation already processed" error

  Scenario: Confirm booking - reservation expired
    Given a "reservationId", "bookingId", and "UserIdentity" with an expired reservation
    When Patient calls "POST /timeslot/v1/confirm"
    Then Patient should get a "ConfirmBookingResponse.NotFound" response
    And Timeslot status should be "Free"

  Scenario: Cancel booking - by reservationId - successful cancellation
    Given a "reservationId" and "UserIdentity" with a Pending or Confirmed reservation
    When Patient calls "POST /timeslot/v1/cancel" with "reservationId"
    Then Patient should get a "CancelBookingResponse.Success" response
    And Reservation status should be "Cancelled"
    And Timeslot status should be "Free"
    And "TimeslotReleased" event should be published to "appointments" topic

  Scenario: Cancel booking - by bookingId - successful cancellation
    Given a "bookingId" and "UserIdentity" with a Confirmed reservation
    When Patient calls "POST /timeslot/v1/cancel" with "bookingId"
    Then Patient should get a "CancelBookingResponse.Success" response
    And Reservation status should be "Cancelled"
    And Timeslot status should be "Free"
    And "TimeslotReleased" event should be published to "appointments" topic

  Scenario: Cancel booking - reservation not found
    Given a "reservationId" or "bookingId" and "UserIdentity" with non-existent reservation
    When Patient calls "POST /timeslot/v1/cancel"
    Then Patient should get a "CancelBookingResponse.NotFound" response

  Scenario: Cancel booking - missing both IDs
    Given a "UserIdentity"
    When Patient calls "POST /timeslot/v1/cancel" without "reservationId" and "bookingId"
    Then Patient should get a "400 Bad Request" response with validation error

  Scenario: Idempotency - duplicate reserve request
    Given a "timeslotId", "UserIdentity", and "correlationId" with a first successful reservation
    When Patient calls "POST /timeslot/v1/reserve" with the same "correlationId"
    Then Patient should get the same "ReserveTimeslotResponse.Success" response
    And Rate limit count should NOT be incremented
    And No new reservation should be created

  Scenario: Query range validation - range exceeds 30 days
    Given a "doctorId", "startTime", "endTime", and "UserIdentity" with range > 30 days
    When Patient calls "GET /timeslot/v1/available"
    Then Patient should get a "400 Bad Request" response with validation error

  Scenario: Query range validation - startTime in past
    Given a "doctorId", "startTime" in the past, and "UserIdentity"
    When Patient calls "GET /timeslot/v1/available"
    Then Patient should get a "400 Bad Request" response with validation error

  Scenario: Unauthorized - missing auth header
    Given no "UserIdentity" header
    When Patient calls "GET /timeslot/v1/available"
    Then Patient should get a "401 Unauthorized" response

  Scenario: Unauthorized - malformed auth header
    Given a malformed "UserIdentity" header
    When Patient calls "GET /timeslot/v1/available"
    Then Patient should get a "401 Unauthorized" response

  Scenario: Expiry - reservation expires and is released automatically
    Given a Pending reservation with short TTL and "Redis"
    When Expiry worker polls Redis sorted set
    Then Reservation should be released via Redis sorted set
    And Reservation status should be "Expired"
    And Timeslot status should be "Free"
    And "TimeslotReleased" event should be published

  Scenario: Expiry - DB fallback catches missed expirations
    Given a Pending reservation that was missed by Redis expiry
    When DB fallback scan runs
    Then Reservation should be released
    And Reservation status should be "Expired"
    And Timeslot status should be "Free"
