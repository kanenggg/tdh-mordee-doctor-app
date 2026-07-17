use crate::core::health::{
    HealthDetailedResponse, HealthLivenessResponse, HealthReadinessResponse, HealthSimpleResponse,
};
use crate::model::consultation_state::{
    ConsultationPhase, ConsultationState, ConsultationStateDoc,
};
use crate::module::notification::fcm_token::{
    FcmTokenDoc, RegisterFcmTokenRequest, RegisterFcmTokenResponse,
};
use crate::module::notification::handlers::NotificationListResponse;
use crate::module::notification::repo::{ScheduledNotificationDoc, ScheduledNotificationStatus};
use crate::module::{
    consultation::{
        routes::EndSessionBody,
        services::{FaceVerificationRequest, SessionChannel, SessionInfo},
    },
    timeslot::handler::{GetAvailableTimeslotsQuery, GetAvailableTimeslotsResponse},
    timeslot::model::{Timeslot, TimeslotStatus},
};
use tdh_protocol::biz_apm::consultation_event::{
    ConsultationBookedEvent, ConsultationCancelledEvent, ConsultationEvent,
    ConsultationSummarizedEvent, Medicine, PatientAcceptedFollowUpEvent, PrescriptionInfo,
    SessionCreatedEvent, SessionParticipant, TerminationCode, TimeslotReservedEvent,
};
use tdh_protocol::biz_apm::{ConsultationChannel, PatientIdentity};
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "TDH-SEC-IAM-USER-IDENTITY",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new(
                    "TDH-SEC-IAM-USER-IDENTITY",
                ))),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        // Health check endpoints
        crate::core::health::health,
        crate::core::health::health_liveness,
        crate::core::health::health_readiness,
        crate::core::health::health_detailed,
        // Notification endpoints
        crate::module::notification::handlers::get_notifications,
        crate::module::notification::handlers::create_notification,
        crate::module::notification::handlers::mark_as_read,
        crate::module::notification::handlers::mark_all_as_read,
        crate::module::notification::handlers::mark_all_as_unread,
        crate::module::notification::handlers::register_fcm_token,
        crate::module::notification::handlers::get_fcm_tokens,
        crate::module::notification::handlers::delete_fcm_token,
        // Consultation endpoints
        crate::module::consultation::routes::submit_face_verification,
        crate::module::consultation::routes::end_session,
        crate::module::consultation::routes::get_session_info,
        // Consultation summarization endpoints
        crate::module::consultation::summarization::handler::get_summary_note,
        crate::module::consultation::summarization::handler::save_draft,
        crate::module::consultation::summarization::handler::submit,
        // Ranking endpoints
        crate::module::ranking::handlers::list_instant_doctors,
        crate::module::ranking::handlers::list_scheduled_doctors,
        crate::module::ranking::handlers::get_doctor_profile,
        // Profile consultation settings endpoints
        crate::module::profile::consultation_setting::handler::update_schedule_config,
        crate::module::profile::consultation_setting::handler::get_schedule_config,
        // Profile availability endpoints
        crate::module::profile::availability::handler::update_schedule_availability,
        crate::module::profile::availability::handler::update_instant_availability,
        crate::module::profile::availability::handler::get_availability,
        // Profile doctor-configuration endpoints
        crate::module::profile::configuration::handler::get_doctor_configuration,
        crate::module::profile::configuration::handler::update_doctor_channel,
        crate::module::profile::configuration::handler::update_doctor_language,
        // Timeslot endpoints
        crate::module::timeslot::handler::get_available_timeslot,
        crate::module::timeslot::get_my_available_time_slots::handler::get_my_available_timeslots,
        // Appointment endpoints
        crate::module::appointment::handler::get_appointment_detail,
        crate::module::ekyc::handlers::get_appointment_ekyc,
        // Onboarding endpoints
        crate::module::onboarding::handler::get_doctor_profile_draft,
        crate::module::onboarding::handler::get_onboarding_status,
        crate::module::onboarding::handler::save_doctor_profile_draft,
        crate::module::onboarding::handler::submit_doctor_profile_draft,
        // Back office endpoints
        crate::module::backoffice::onboarding::pending::list_pending_approvals,
        crate::module::backoffice::onboarding::pending::get_pending_approval,
        crate::module::backoffice::doctor_management::handler::update_consultation_configuration,
        crate::module::backoffice::onboarding::handler::approve_doctor,
        crate::module::backoffice::onboarding::handler::reject_doctor,
        crate::module::backoffice::doctor_management::handler::update_doctor_active_status,
        crate::module::backoffice::doctor_directory::handler::list_approved_doctors,
        crate::module::backoffice::doctor_directory::handler::get_approved_doctor,
        // Profile endpoints
        crate::module::profile::handler::get_profile,
        crate::module::profile::handler::get_doctor_id,
        // EHR endpoints
        crate::module::ehr::lab_result::handler::get_lab_result,
        crate::module::ehr::past_visit_history::handler::get_past_visits,
        crate::module::ehr::past_visit_detail::handler::get_past_visit_detail,
    ),
    components(
        schemas(
            // Health check
            HealthSimpleResponse,
            HealthLivenessResponse,
            HealthReadinessResponse,
            HealthDetailedResponse,
            // Notification
            NotificationListResponse,
            RegisterFcmTokenRequest,
            FcmTokenDoc,
            RegisterFcmTokenResponse,
            // Consultation
            FaceVerificationRequest,
            EndSessionBody,
            SessionInfo,
            SessionChannel,
            // Consultation summarization - DTOs (request/response)
            crate::module::consultation::summarization::handler::GetSummarizationResponse,
            crate::module::consultation::summarization::handler::SaveDraftRequest,
            crate::module::consultation::summarization::handler::SaveDraftResult,
            crate::module::consultation::summarization::handler::SubmitRequest,
            crate::module::consultation::summarization::handler::SubmitResponse,
            // Consultation summarization - Domain models
            crate::module::consultation::summarization::models::SummarizationRecord,
            crate::module::consultation::summarization::models::SummaryNote,
            crate::module::consultation::summarization::models::Prescription,
            crate::module::consultation::summarization::models::PrescriptionOption,
            crate::module::consultation::summarization::models::PrescriptionItem,
            crate::module::consultation::summarization::models::FollowUpInfo,
            crate::module::consultation::summarization::models::DrugAllergyInfo,
            crate::module::consultation::summarization::models::DrugAllergy,
            crate::module::consultation::summarization::models::IllnessDuration,
            crate::module::consultation::summarization::models::RefData,
            crate::module::consultation::summarization::models::RefDataWithAmount<i32>,
            crate::module::consultation::summarization::models::SummarizationStatus,
            ScheduledNotificationDoc,
            ScheduledNotificationStatus,
            // Consultation Events / State
            ConsultationEvent,
            ConsultationStateDoc,
            ConsultationState,
            ConsultationPhase,
            PatientIdentity,
            ConsultationChannel,
            PrescriptionInfo,
            Medicine,
            SessionParticipant,
            TerminationCode,
            TimeslotReservedEvent,
            ConsultationBookedEvent,
            ConsultationCancelledEvent,
            SessionCreatedEvent,
            PatientAcceptedFollowUpEvent,
            ConsultationSummarizedEvent,
            // Ranking
            crate::module::ranking::models::DoctorListResponse,
            crate::module::ranking::models::DoctorProfileResponse,
            crate::module::ranking::models::DoctorProfile,
            crate::module::ranking::models::DoctorListData,
            crate::module::ranking::models::PagingMetaData,
            crate::module::ranking::models::LocalizedName,
            crate::module::ranking::models::SpecialtyInfo,
            crate::module::ranking::models::ChannelInfo,
            crate::module::ranking::models::WorkplaceInfo,
            crate::module::ranking::models::AssociatePrivilege,
            crate::module::ranking::models::PolicyType,
            // Profile consultation settings
            crate::module::profile::consultation_setting::model::TimePeriod,
            crate::module::profile::consultation_setting::model::DateWithTimePeriods,
            crate::module::profile::consultation_setting::model::ScheduleAvailableConfig,
            crate::module::profile::consultation_setting::model::UpdateScheduleConfigResponse,
            crate::module::profile::common::BizUnitQuery,
            crate::module::profile::common::SuccessResponse,
            crate::module::profile::availability::models::UpdateAvailabilityRequest,
            crate::module::profile::availability::models::AvailabilityResponse,
            // Profile doctor-configuration
            crate::module::profile::configuration::models::ChannelType,
            crate::module::profile::configuration::models::LanguageCode,
            crate::module::profile::configuration::models::Fee,
            crate::module::profile::configuration::models::DoctorConfiguration,
            crate::module::profile::configuration::models::GetDoctorConfigurationResponse,
            crate::module::profile::configuration::models::UpdateChannelRequest,
            crate::module::profile::configuration::models::UpdateLanguageRequest,
            crate::module::profile::configuration::models::UpdateConfigurationResponse,
            // Profile
            crate::module::profile::handler::DoctorProfileResponse,
            crate::module::profile::handler::DoctorIdResponse,
            // Timeslot
            Timeslot,
            TimeslotStatus,
            GetAvailableTimeslotsQuery,
            GetAvailableTimeslotsResponse,
            crate::module::timeslot::handler::MyAvailableQuery,
            crate::module::timeslot::handler::MyAvailableResponse,
            crate::module::timeslot::handler::DoctorTimeslotSchema,
            // Appointment
            crate::module::appointment::model::ApiResponse,
            crate::module::appointment::model::SuccessBody,
            crate::module::appointment::model::AppointmentTime,
            crate::module::appointment::model::Patient,
            crate::module::appointment::model::Payment,
            crate::module::appointment::model::Coupon,
            crate::module::appointment::model::Prescreen,
            crate::module::appointment::model::PastVisitApiResponse,
            crate::module::appointment::model::PastVisitResponse,
            crate::module::appointment::model::PastVisitsListResponse,
            crate::model::appointment_status::AppointmentCardStatus,
            // eKYC
            crate::module::ekyc::handlers::EkycResponse,
            // Onboarding
            crate::module::onboarding::handler::GetOnboardingResponse,
            crate::module::onboarding::handler::OnboardingStatusResponse,
            crate::model::onboarding::OnBoarding,
            crate::model::onboarding::OnBoardingStatus,
            crate::model::onboarding::OnBoardingRequestPatch,
            crate::model::onboarding::SelectedWorkPlaceRequest,
            crate::model::onboarding::EducationRequest,
            crate::model::onboarding::Address,
            crate::model::onboarding::SelectedWorkPlace,
            crate::model::onboarding::Education,
            crate::model::onboarding::Documents,
            crate::model::onboarding::Specialty,
            crate::model::onboarding::Subspecialty,
            // Back office
            crate::module::backoffice::onboarding::pending::PendingDoctorApprovalListResponse,
            crate::module::backoffice::onboarding::pending::PendingDoctorApprovalSummary,
            crate::module::backoffice::onboarding::pending::PendingDoctorApprovalDetailResponse,
            crate::module::backoffice::onboarding::pending::PendingDoctorApprovalDetail,
            crate::module::backoffice::onboarding::pending::PendingDoctorApprovalAddress,
            crate::module::backoffice::consultation_configuration::DoctorConsultationConfig,
            crate::module::backoffice::onboarding::handler::ApproveRequest,
            crate::module::backoffice::doctor_management::handler::UpdateConsultationConfigurationRequest,
            crate::module::backoffice::onboarding::handler::RejectRequest,
            crate::module::backoffice::doctor_management::handler::UpdateDoctorActiveStatusRequest,
            crate::module::backoffice::doctor_directory::handler::ApprovedDoctorListResponse,
            crate::module::backoffice::doctor_directory::handler::ApprovedDoctorDetailResponse,
            crate::module::backoffice::doctor_directory::repo::ApprovedDoctorSummary,
            crate::module::backoffice::doctor_directory::repo::ApprovedDoctorDetail,
            // EHR lab-result
            crate::module::ehr::lab_result::handler::LabResult,
            crate::module::ehr::lab_result::handler::GetLabResultsResult,
            // EHR past-visit history (list)
            crate::module::ehr::past_visit_history::handler::PastVisit,
            crate::module::ehr::past_visit_history::handler::PastVisitDoctorInfo,
            crate::module::ehr::past_visit_history::handler::GetPastVisitHistoryResult,
            // EHR past-visit-detail
            crate::module::ehr::past_visit_detail::handler::GetPastVisitDetailResult,
            crate::module::ehr::past_visit_detail::handler::PastVisitDetailResponse,
            crate::module::ehr::past_visit_detail::handler::TimeRange,
            crate::module::ehr::past_visit_detail::handler::DoctorInfo,
            crate::module::ehr::past_visit_detail::handler::SummaryNote,
            crate::module::ehr::past_visit_detail::handler::Icd10,
            crate::module::ehr::past_visit_detail::handler::DrugAllergyInfo,
            crate::module::ehr::past_visit_detail::handler::DrugAllergy,
            crate::module::ehr::past_visit_detail::handler::IllnessDuration,
            crate::module::ehr::past_visit_detail::handler::PrescriptionItem,
            crate::module::ehr::past_visit_detail::handler::FollowUp,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "notifications", description = "Doctor notifications"),
        (name = "consultation", description = "Doctor consultation session"),
        (name = "tasks", description = "Cloud Tasks scheduled notification endpoints"),
        (name = "ranking", description = "Doctor ranking and listing endpoints"),
        (name = "profile", description = "Doctor profile consultation settings"),
        (name = "timeslot", description = "Doctor timeslot management"),
        (name = "appointment", description = "Doctor appointment detail"),
        (name = "onboarding", description = "Doctor onboarding"),
        (name = "back office", description = "Doctor back office"),
        (name = "ehr", description = "Electronic health record endpoints"),
    ),
    modifiers(&SecurityAddon),
    info(
        title = "Mordee Doctor API",
        version = "1.0.0",
        description = "REST API for Mordee doctor application"
    )
)]
pub struct ApiDoc;
