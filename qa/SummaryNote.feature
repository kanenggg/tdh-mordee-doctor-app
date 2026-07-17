Feature: SummaryNote

  Scenario: Get summarization - no draft exists (PendingRecord)
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "GET /consultation/v1/summarization/:appointment_id"
    Then Doctor should get a "GetSummarizationResponse.PendingRecord" response

  Scenario: Get summarization - existing draft
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "GET /consultation/v1/summarization/:appointment_id"
    Then Doctor should get a "GetSummarizationResponse.SummarizationRecord" with status "Draft" response

  Scenario: Get summarization - already submitted
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "GET /consultation/v1/summarization/:appointment_id"
    Then Doctor should get a "GetSummarizationResponse.SummarizationRecord" with status "Submitted" response

  Scenario: Save summary note as draft
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/draft"
    Then Doctor should get a "SaveDraftResult.Success" response

  Scenario: Save summary note - already submitted
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/draft"
    Then Doctor should get a "SaveDraftResult.AlreadySubmitted" response

  Scenario: Save partial draft - only summary_note
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/draft" with only "summary_note" field
    Then Doctor should get a "SaveDraftResult.Success" response

  Scenario: Save partial draft - only prescription
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/draft" with only "prescription" field
    Then Doctor should get a "SaveDraftResult.Success" response

  Scenario: Save partial draft - only follow_up_info
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/draft" with only "follow_up_info" field
    Then Doctor should get a "SaveDraftResult.Success" response

  Scenario: Submit summary note
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit"
    Then Doctor should get a "SubmitResponse.Success" response

  Scenario: Re-submit summary note
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit"
    Then Doctor should get a "SubmitResponse.AlreadySubmitted" response

  Scenario: Submit with missing summary_note field
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" without "summary_note" field
    Then Doctor should get a "400 Bad Request" response with validation error

  Scenario: Submit with missing prescription field
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" without "prescription" field
    Then Doctor should get a "400 Bad Request" response with validation error

  Scenario: Submit with missing follow_up_info field
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" without "follow_up_info" field
    Then Doctor should get a "400 Bad Request" response with validation error

  Scenario: Submit summary note with prescription
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" with prescription items
    Then biz-jade should be called to create prescription with "biz_unit_id", "biz_center_id", and "patient_id"
    And biz-apm should be called to save summary note with "prescription_ref"
    And ConsultationSummarized event should be published
    And Doctor should get a "SubmitResponse.Success" response

  Scenario: Submit summary note without prescription
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" without prescription
    Then biz-jade should NOT be called
    And biz-apm should be called to save summary note without "prescription_ref"
    And ConsultationSummarized event should be published
    And Doctor should get a "SubmitResponse.Success" response

  Scenario: Submit summary note - biz-apm returns AlreadySubmitted
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit"
    And biz-apm returns "AlreadySubmitted" for summary note
    Then ConsultationSummarized event should NOT be published
    And Doctor should get a "SubmitResponse.AlreadySubmitted" response

  Scenario: Submit summary note - biz-jade service error
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit" with prescription items
    And biz-jade service returns an error
    Then biz-apm should NOT be called
    And ConsultationSummarized event should NOT be published
    And Doctor should get a "SubmitResponse.PrescriptionServiceError" response with error message

  Scenario: Submit summary note - biz-apm service error
    Given an "AppointmentId", and "UserIdentity"
    When Doctor call "POST /consultation/v1/summarization/submit"
    And biz-apm service returns an error
    And summary note was not already submitted
    Then ConsultationSummarized event should NOT be published
    And Doctor should get a "SubmitResponse.ConsultationServiceError" response with error message

