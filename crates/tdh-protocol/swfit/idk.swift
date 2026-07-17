import Foundation
import Core

public struct AppointmentDetailDTO: Decodable, Sendable {

  // MARK: - Nested DTOs

  public struct StatusDTO: Decodable, Sendable {
    public let __type: String
    public let summaryNote: SummaryNoteDTO?

    public let prescriptionItems: [PrescriptionItemDTO]?
    public let followUpInfo: FollowUpInfoDTO?
  }

  public struct PayerDTO: Decodable, Sendable {
    public let __type: String
    public let company: String?
    public let insuranceCondition: String?
  }

  public struct AppointmentTimeDTO: Decodable, Sendable {
    public let startTime: String
    public let endTime: String
  }

  public struct PatientDTO: Decodable, Sendable {
    public let accountId: Int
    public let profileId: Int
    public let fullName: String
    public let dateOfBirth: String
    public let gender: String
    public let bmi: Double?
    public let weight: Double?
  }

  public struct SymptomsDurationDTO: Decodable, Sendable {
    public let value: Int?
    public let unit: String?
  }

  public struct SymptomsDTO: Decodable, Sendable {
    public let description: String
    public let duration: SymptomsDurationDTO?
    public let drugAllergies: [String]?
  }

  // MARK: - Summary Note DTOs (inside status)

  public struct SummaryNoteDTO: Decodable, Sendable {

    public struct ICD10DTO: Decodable, Sendable {
      public let code: String
      public let description: String
    }

    public struct IllnessDurationDTO: Decodable, Sendable {
      public let value: Int?
      public let unit: String?
    }

    public struct DrugAllergyInfoDTO: Decodable, Sendable {
      public let __type: String
      public let drugAllergies: [DrugAllergyDTO]?
    }

    public struct DrugAllergyDTO: Decodable, Sendable {
      public let id: Int
      public let description: String
    }

    public let presentIllness: String
    public let chiefComplaint: String
    public let diagnosis: String
    public let recommendations: String
    public let icd10: [ICD10DTO]
    public let drugAllergyInfo: DrugAllergyInfoDTO?
    public let illnessDuration: IllnessDurationDTO?
    public let noteToStaff: String?
  }

  // MARK: - NEW: prescription + follow-up DTOs (inside status)

  public struct PrescriptionItemDTO: Decodable, Sendable {
    public let medId: Int
    public let name: String
    public let quantity: Int
    public let unit: String
    public let dosageInstructions: String
  }

  public struct FollowUpTimeDTO: Decodable, Sendable {
    public let startTime: String
    public let endTime: String
  }

  public struct FollowUpInfoDTO: Decodable, Sendable {
    public let __type: String               // "ScheduleAppointment" | "AsNeeded"
    public let followUpDate: String?        // "yyyy-MM-dd"
    public let followUpTime: FollowUpTimeDTO?
    public let visitType: String?           // "FollowUp" / "LabResults" / ...
    public let noteToPatient: String?
    public let noteToStaff: String?
  }

  // MARK: - Root fields

  public let appointmentId: String
  public let appointmentDate: String
  public let appointmentTime: AppointmentTimeDTO
  public let status: StatusDTO
  public let payer: PayerDTO?
  public let patient: PatientDTO
  public let symptom: SymptomsDTO?
  public let attachments: [String]?
  public let channel: String?

  enum CodingKeys: String, CodingKey {
    case appointmentId
    case appointmentDate
    case appointmentTime
    case status
    case payer
    case patient
    case symptom
    case attachments
    case channel
  }
}

// MARK: - SummaryNoteDTO -> SummaryNoteBody

public extension AppointmentDetailDTO.SummaryNoteDTO {

  func toDomain() -> SummaryNoteBody {
    // ICD10
    let icdEntries: [SummaryNoteCodeDescription] = icd10.map {
      SummaryNoteCodeDescription(
        code: $0.code,
        description: $0.description
      )
    }

    // Duration
    let illnessDurationDomain: SummaryNoteDuration? = {
      guard
        let d = illnessDuration,
        let value = d.value,
        let unitStr = d.unit,
        let unit = SummaryNoteDurationUnit(rawValue: unitStr)
      else {
        return nil
      }
      return SummaryNoteDuration(value: value, unit: unit)
    }()

    // Drug allergies
    let allergyInfo: SummaryNoteDrugAllergyInfo = {
      guard let info = drugAllergyInfo else {
        return .noKnownAllergies
      }

      switch info.__type {
      case "HasDrugAllergies":
        let arr = info.drugAllergies ?? []
        let domainAllergies: [SummaryNoteDrugAllergy] = arr.map {
          SummaryNoteDrugAllergy(
            id: $0.id,
            description: $0.description
          )
        }
        return domainAllergies.isEmpty
          ? .noKnownAllergies
          : .hasDrugAllergies(domainAllergies)

      case "NoDrugAllergies":
        return .noKnownAllergies

      default:
        return .noKnownAllergies
      }
    }()

    return SummaryNoteBody(
      presentIllness: presentIllness,
      chiefComplaint: chiefComplaint,
      diagnosis: diagnosis,
      recommendations: recommendations,
      icd10: icdEntries,
      drugAllergyInfo: allergyInfo,
      illnessDuration: illnessDurationDomain,
      noteToStaff: noteToStaff ?? ""
    )
  }
}

// MARK: - Mapping to domain

public extension AppointmentDetailDTO {

  func toDomain() -> AppointmentDetail {
    // MARK: - Parse start/end Date from (date + time)

    let dateTimeFormatter: DateFormatter = {
      let f = DateFormatter()
      f.calendar = Calendar(identifier: .gregorian)
      f.locale = Locale(identifier: "en_US_POSIX")
      f.timeZone = .current
      f.dateFormat = "yyyy-MM-dd HH:mm"
      return f
    }()

    let startDate: Date
    let endDate: Date

    if let s = dateTimeFormatter.date(
      from: "\(appointmentDate) \(appointmentTime.startTime)"
    ), let e = dateTimeFormatter.date(
      from: "\(appointmentDate) \(appointmentTime.endTime)"
    ) {
      startDate = s
      endDate = e
    } else {
      startDate = Date()
      endDate = startDate.addingTimeInterval(15 * 60)
    }

    let durationSec = max(0, endDate.timeIntervalSince(startDate))

    let schedule = AppointmentSchedule(
      startDate: startDate,
      durationSec: durationSec
    )

    // MARK: - Status mapping from status.__type

    let backendStatus: AppointmentStatus = {
      switch status.__type.lowercased() {
      case "upcoming":
        return .comingUp
      case "ongoing":
        return .ongoing
      case "completed":
        return .completed
      case "missed":
        return .missed
      case "pendingrecord":
        return .record
      default:
        return .comingUp
      }
    }()

    // MARK: - FRONTEND OVERRIDE: upcoming -> ongoing if within time range

    let now = Date()

    let effectiveStatus: AppointmentStatus = {
      if backendStatus == .comingUp,
         now >= schedule.startDate,
         now <= schedule.endDate {
        return .ongoing
      }
      return backendStatus
    }()

    // MARK: - Channel mapping

    let domainChannel: ConsultationChannel = {
      guard let ch = channel?.uppercased() else {
        return .video
      }
      switch ch {
      case "VIDEO": return .video
      case "VOICE": return .voice
      case "CHAT":  return .chat
      default:      return .video
      }
    }()

    // MARK: - Gender & DOB

    let domainGender: Gender = {
      switch patient.gender.lowercased() {
      case "male":   return .male
      case "female": return .female
      default:       return .undefined
      }
    }()

    let dobFormatter: DateFormatter = {
      let f = DateFormatter()
      f.calendar = Calendar(identifier: .gregorian)
      f.locale = Locale(identifier: "en_US_POSIX")
      f.timeZone = .current
      f.dateFormat = "yyyy-MM-dd"
      return f
    }()

    let dob = dobFormatter.date(from: patient.dateOfBirth)
      ?? Date(timeIntervalSince1970: 0)

    let payerName = payer?.company
    let insuranceURL: URL? = payer?.insuranceCondition.flatMap { URL(string: $0) }

    let domainPatient = Patient(
      accountId: patient.accountId,
      profileId: patient.profileId,
      fullName: patient.fullName,
      gender: domainGender,
      dateOfBirth: dob,
      payerName: payerName,
      insuranceConditionURL: insuranceURL
    )

    // MARK: - Body metrics (BMI / weight from patient)

    let domainBody: BodyMetrics? = {
      if patient.bmi != nil || patient.weight != nil {
        return BodyMetrics(bmi: patient.bmi, weightKg: patient.weight)
      } else {
        return nil
      }
    }()

    // MARK: - Symptoms

    let domainSymptoms: ClinicalSymptoms = {
      let desc = symptom?.description ?? ""
      let period: String = {
        if let dur = symptom?.duration,
           let value = dur.value,
           let unit = dur.unit {
          return "\(value) \(unit)"
        } else {
          return ""
        }
      }()
      let allergies = symptom?.drugAllergies ?? []
      return ClinicalSymptoms(
        primaryProblem: desc,
        periodOfSickness: period,
        drugAllergies: allergies
      )
    }()

    // MARK: - Attachments

    let domainAttachments: Attachment? = {
      guard let attachments, !attachments.isEmpty else { return nil }
      let urls = attachments.compactMap { URL(string: $0) }
      guard !urls.isEmpty else { return nil }
      return Attachment(id: appointmentId, imageURLs: urls)
    }()

    // MARK: - Summary note

    let domainSummaryNote: SummaryNoteBody? = status.summaryNote?.toDomain()

    // MARK: - Backend prescriptionItems -> domain SummaryNotePrescriptionItem

    let backendPrescription: [SummaryNotePrescriptionItem] = {
      guard let items = status.prescriptionItems else { return [] }

      return items.map { dto in
        let dose = SummaryNoteDose(value: 0, unit: dto.unit)

        let route = SummaryNoteIdDescription(
          id: 0,
          description: ""
        )

        let frequency = SummaryNoteIdDescription(
          id: 0,
          description: dto.dosageInstructions
        )

        let indication = SummaryNoteIdDescription(
          id: 0,
          description: ""
        )

        let foodTiming = SummaryNoteIdDescription(
          id: 0,
          description: ""
        )

        let duration = SummaryNoteDuration(
          value: 0,
          unit: .days
        )

        return SummaryNotePrescriptionItem(
          medicineId: dto.medId,
          medicineName: dto.name,
          dose: dose,
          quantity: dto.quantity,
          route: route,
          frequency: frequency,
          indication: indication,
          foodTiming: foodTiming,
          duration: duration,
          cautions: "",
          remark: "",
          noteToPatient: ""
        )
      }
    }()

    // MARK: - Backend followUpInfo -> domain SummaryNoteFollowUp

    let backendFollowUp: SummaryNoteFollowUp? = {
      guard let info = status.followUpInfo else { return nil }

      switch info.__type {
      case "ScheduleAppointment":
        guard
          let dateStr  = info.followUpDate,
          let time     = info.followUpTime,
          let visitStr = info.visitType
        else {
          return nil
        }

        let scheduled = SummaryNoteScheduledFollowUp(
          followUpDate: dateStr,
          startTime: time.startTime,
          endTime: time.endTime,
          visitType: visitStr,
          noteToPatient: info.noteToPatient ?? "",
          noteToStaff: info.noteToStaff ?? ""
        )
        return .scheduleAppointment(scheduled)

      case "AsNeeded":
        return .asNeeded(noteToStaff: info.noteToStaff ?? "")

      default:
        return nil
      }
    }()

    // MARK: - Construct domain

    return AppointmentDetail(
      id: appointmentId,
      patient: domainPatient,
      schedule: schedule,
      status: effectiveStatus,
      channel: domainChannel,
      body: domainBody,
      symptoms: domainSymptoms,
      attachments: domainAttachments,
      summaryNote: domainSummaryNote,
      summaryPrescriptionItems: backendPrescription,
      summaryFollowUp: backendFollowUp
    )
  }
}

