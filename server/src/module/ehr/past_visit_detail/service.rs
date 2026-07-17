use jiff::{tz::TimeZone, Timestamp};
use std::sync::Arc;

use crate::core::error::AppResult;
use crate::model::localize::Localized;

use super::gateway::{
    ApmConsultationChannel, ApmDoctorRef, ApmDrugAllergy, ApmDurationUnit, ApmFollowUp,
    ApmFollowUpAppointment, ApmIcd10, ApmPastVisitDetail, ApmPastVisitSummaryNote, ApmVisitType,
    JadePrescriptionItem, PastVisitDetailBundle, PastVisitDetailFromGateway, PastVisitGateway,
};
use super::handler::{
    DoctorInfo, DrugAllergy, DrugAllergyInfo, FollowUp, GetPastVisitDetailResult, Icd10,
    IllnessDuration, PastVisitDetailResponse, PrescriptionItem, SummaryNote, TimeRange,
};
use super::repo::{DoctorBasicInfo, DoctorBasicRepoTrait};

const TIMEZONE: &str = "Asia/Bangkok";

#[derive(Clone)]
pub struct PastVisitDetailService {
    gateway: Arc<PastVisitGateway>,
    doctor_repo: Arc<dyn DoctorBasicRepoTrait>,
}

impl PastVisitDetailService {
    pub fn new(gateway: Arc<PastVisitGateway>, doctor_repo: Arc<dyn DoctorBasicRepoTrait>) -> Self {
        Self {
            gateway,
            doctor_repo,
        }
    }

    pub async fn get_past_visit_detail(
        &self,
        request_id: &str,
        booking_id: &str,
    ) -> AppResult<GetPastVisitDetailResult> {
        let bundle = self
            .gateway
            .get_past_visit_detail(request_id, booking_id)
            .await?;

        Ok(match bundle {
            PastVisitDetailFromGateway::Success(b) => {
                let doctor_basic = self
                    .doctor_repo
                    .get_doctor_basic(b.detail.doctor.doctor_account_id)
                    .await?;
                GetPastVisitDetailResult::PastVisitDetail(map_bundle(b, doctor_basic))
            }
            PastVisitDetailFromGateway::NotFound => GetPastVisitDetailResult::NotFound,
            PastVisitDetailFromGateway::NotFulfilled => GetPastVisitDetailResult::NotFulfilled,
        })
    }
}

fn map_bundle(
    b: PastVisitDetailBundle,
    doctor_basic: Option<DoctorBasicInfo>,
) -> PastVisitDetailResponse {
    let PastVisitDetailBundle {
        detail,
        prescription,
    } = b;
    map_detail(detail, doctor_basic, prescription)
}

fn map_detail(
    d: ApmPastVisitDetail,
    doctor_basic: Option<DoctorBasicInfo>,
    prescription: Vec<JadePrescriptionItem>,
) -> PastVisitDetailResponse {
    PastVisitDetailResponse {
        appointment_id: d.booking_id,
        appointment_date: date_from_epoch(d.appointment_time.start_time),
        appointment_time: TimeRange {
            start_time: time_from_epoch(d.appointment_time.start_time),
            end_time: time_from_epoch(d.appointment_time.end_time),
        },
        consultation_channel: channel_to_string(d.consultation_channel),
        doctor: map_doctor(d.doctor, doctor_basic),
        summary_note: map_summary_note(d.summary_note),
        prescription_items: prescription.into_iter().map(map_prescription).collect(),
        follow_up: map_follow_up(d.follow_up),
    }
}

fn channel_to_string(c: ApmConsultationChannel) -> String {
    match c {
        ApmConsultationChannel::Video => "Video",
        ApmConsultationChannel::Voice => "Voice",
        ApmConsultationChannel::Chat => "Chat",
    }
    .to_string()
}

// APM's DoctorRef carries only IDs; name and specialties are merged from the
// doctor profile table and flattened to display strings (prefer English).
fn map_doctor(d: ApmDoctorRef, basic: Option<DoctorBasicInfo>) -> DoctorInfo {
    let (name, specialties) = match basic {
        Some(b) => (
            format_doctor_name(&b.first_name, &b.last_name),
            b.specialty
                .map(|s| pick_localized(&s.name))
                .into_iter()
                .collect(),
        ),
        None => (String::new(), Vec::new()),
    };
    DoctorInfo {
        id: d.doctor_id.to_string(),
        name,
        specialties,
    }
}

fn format_doctor_name(first: &Localized, last: &Localized) -> String {
    let first = pick_localized(first);
    let last = pick_localized(last);
    match (first.is_empty(), last.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("Dr. {}", first),
        (true, false) => format!("Dr. {}", last),
        (false, false) => format!("Dr. {} {}", first, last),
    }
}

// Default to English; fall back to Thai when English is empty.
fn pick_localized(loc: &Localized) -> String {
    if !loc.en.is_empty() {
        loc.en.clone()
    } else {
        loc.th.clone()
    }
}

fn map_summary_note(n: ApmPastVisitSummaryNote) -> SummaryNote {
    SummaryNote {
        present_illness: n.present_illness,
        chief_complaint: n.chief_complaint,
        diagnosis: n.diagnosis,
        recommendations: n.recommendations,
        icd10: n.icd10.into_iter().map(map_icd10).collect(),
        drug_allergy_info: map_drug_allergies(n.drug_allergies),
        illness_duration: map_duration(n.illness_duration),
        note_to_staff: n.note_to_staff,
    }
}

fn map_icd10(i: ApmIcd10) -> Icd10 {
    Icd10 {
        code: i.code,
        description: i.description,
    }
}

fn map_drug_allergies(allergies: Option<Vec<ApmDrugAllergy>>) -> DrugAllergyInfo {
    match allergies {
        None => DrugAllergyInfo::NoDrugAllergies,
        Some(list) if list.is_empty() => DrugAllergyInfo::NoDrugAllergies,
        Some(list) => DrugAllergyInfo::HasDrugAllergies {
            drug_allergies: list
                .into_iter()
                .map(|a| DrugAllergy {
                    id: a.id,
                    description: a.display_name,
                })
                .collect(),
        },
    }
}

fn map_duration(d: ApmDurationUnit) -> IllnessDuration {
    IllnessDuration {
        value: d.value,
        unit: d.unit,
    }
}

fn map_prescription(p: JadePrescriptionItem) -> PrescriptionItem {
    PrescriptionItem {
        med_id: p.med_id,
        name: p.name,
        quantity: p.quantity,
        unit: p.unit,
        dosage_instructions: p.dosage_instructions,
    }
}

fn map_follow_up(f: ApmFollowUp) -> FollowUp {
    match f {
        ApmFollowUp::AsNeeded => FollowUp::AsNeeded {
            note_to_staff: String::new(),
        },
        ApmFollowUp::Appointment(a) => map_follow_up_appointment(a),
    }
}

fn map_follow_up_appointment(a: ApmFollowUpAppointment) -> FollowUp {
    let visit_type = a
        .visit_types
        .first()
        .map(visit_type_to_string)
        .unwrap_or_default();
    FollowUp::ScheduleAppointment {
        follow_up_date: date_from_epoch(a.appointment_start),
        follow_up_time: TimeRange {
            start_time: time_from_epoch(a.appointment_start),
            end_time: time_from_epoch(a.appointment_end),
        },
        visit_type,
        note_to_patient: a.additional_note_to_patient,
        note_to_staff: a.note_to_staff,
    }
}

fn visit_type_to_string(v: &ApmVisitType) -> String {
    match v {
        ApmVisitType::FollowUp => "FollowUp",
        ApmVisitType::LabResult => "LabResult",
        ApmVisitType::PrecriptionRefill => "PrescriptionRefill",
    }
    .to_string()
}

fn date_from_epoch(epoch_secs: i64) -> String {
    Timestamp::from_second(epoch_secs)
        .ok()
        .and_then(|ts| {
            ts.in_tz(TIMEZONE)
                .ok()
                .map(|zdt| zdt.strftime("%Y-%m-%d").to_string())
        })
        .unwrap_or_else(|| {
            Timestamp::now()
                .to_zoned(TimeZone::UTC)
                .strftime("%Y-%m-%d")
                .to_string()
        })
}

fn time_from_epoch(epoch_secs: i64) -> String {
    Timestamp::from_second(epoch_secs)
        .ok()
        .and_then(|ts| {
            ts.in_tz(TIMEZONE)
                .ok()
                .map(|zdt| zdt.strftime("%H:%M").to_string())
        })
        .unwrap_or_else(|| "--:--".to_string())
}
