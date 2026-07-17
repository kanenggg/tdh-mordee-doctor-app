use backoff::ExponentialBackoff;
use dotenvy::dotenv;
use jiff::{Span, Zoned};
use rand::Rng;
use server::config::AppConfig;
use server::module::notification::repo::NotificationDoc;
use server::repo::firestore_repo::{FirestoreRepo, FirestoreRepoTrait};
use std::time::Duration as StdDuration;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DOCTOR_ID: &str = "2443";
const COLLECTION_NAME: &str = "notifications";

/// Generate test notification data for doctorId 2443
/// Creates 50 Alert notifications and 50 Announcement notifications
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install AWS LC rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    // Load .env file for local development
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("generate_test_notifications=info".parse()?),
        )
        .init();

    info!(
        "Starting test notification generation for doctorId: {}",
        DOCTOR_ID
    );

    // Load configuration
    let cfg = AppConfig::load_from_dir(None)?;
    info!("Configuration loaded successfully");

    // Initialize Firestore with retry config
    let retry_config = ExponentialBackoff {
        initial_interval: StdDuration::from_millis(cfg.retry.base_delay_ms),
        max_interval: StdDuration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore = FirestoreRepo::new(&cfg.firestore, retry_config).await?;
    info!("Firestore connection established");

    let base_time = Zoned::now();

    let doctor_id_num: i32 = DOCTOR_ID.parse().unwrap_or(0);

    // Generate 60 Alert notifications
    info!("Generating 60 Alert notifications...");
    for i in 0..60 {
        let notification = generate_alert_notification(i, &base_time);
        let notification_id = notification.notification_id().to_string();
        let doc = to_doc_with_doctor_id(&notification, doctor_id_num);

        firestore
            .set_doc(COLLECTION_NAME, &notification_id, &doc)
            .await?;

        info!("Created Alert {}/60: {}", i + 1, notification_id);
        sleep(StdDuration::from_millis(50)).await;
    }

    // Generate 60 Announcement notifications
    info!("Generating 60 Announcement notifications...");
    for i in 0..60 {
        let notification = generate_announcement_notification(i, &base_time);
        let notification_id = notification.notification_id().to_string();
        let doc = to_doc_with_doctor_id(&notification, doctor_id_num);

        firestore
            .set_doc(COLLECTION_NAME, &notification_id, &doc)
            .await?;

        info!("Created Announcement {}/60: {}", i + 1, notification_id);
        sleep(StdDuration::from_millis(50)).await;
    }

    // Generate 30 long-text Alert notifications
    info!("Generating 30 long-text Alert notifications...");
    for i in 0..30 {
        let notification = generate_long_text_alert_notification(i, &base_time);
        let notification_id = notification.notification_id().to_string();
        let doc = to_doc_with_doctor_id(&notification, doctor_id_num);

        firestore
            .set_doc(COLLECTION_NAME, &notification_id, &doc)
            .await?;

        info!("Created long-text Alert {}/30: {}", i + 1, notification_id);
        sleep(StdDuration::from_millis(50)).await;
    }

    // Generate 30 long-text Announcement notifications
    info!("Generating 30 long-text Announcement notifications...");
    for i in 0..30 {
        let notification = generate_long_text_announcement_notification(i, &base_time);
        let notification_id = notification.notification_id().to_string();
        let doc = to_doc_with_doctor_id(&notification, doctor_id_num);

        firestore
            .set_doc(COLLECTION_NAME, &notification_id, &doc)
            .await?;

        info!(
            "Created long-text Announcement {}/30: {}",
            i + 1,
            notification_id
        );
        sleep(StdDuration::from_millis(50)).await;
    }

    info!(
        "✅ Successfully generated 180 test notifications for doctorId {}",
        DOCTOR_ID
    );
    info!("  120 standard + 60 long-text (90 Alert, 90 Announcement)");
    info!("You can now test pagination at:");
    info!("  GET /notifications/v1?page=1&limit=20");
    info!("  GET /notifications/v1?page=2&limit=20");
    info!("  GET /notifications/v1?type=Alert");
    info!("  GET /notifications/v1?type=Announcement&category=Pharmacy");
    info!("  GET /notifications/v1?type=Announcement&category=Regulation");
    info!("  GET /notifications/v1?type=Announcement&category=Marketing");
    info!("  GET /notifications/v1?type=Announcement&category=Tech%20Update");
    info!("  GET /notifications/v1?type=Announcement&category=Accounting");
    info!("  GET /notifications/v1?type=Announcement&category=Other");

    Ok(())
}

/// Serialize a NotificationDoc and inject the doctorId field for flat collection storage
fn to_doc_with_doctor_id(notification: &NotificationDoc, doctor_id: i32) -> serde_json::Value {
    let mut val = serde_json::to_value(notification).expect("serialize notification");
    if let serde_json::Value::Object(map) = &mut val {
        map.insert("doctorId".to_string(), serde_json::json!(doctor_id));
    }
    val
}

/// Generate an Alert notification with varied content
fn generate_alert_notification(index: usize, base_time: &Zoned) -> NotificationDoc {
    let mut rng = rand::rng();

    // Generate timestamp spread over last 30 days
    let days_ago = rng.random_range(0..30);
    let hours_ago = rng.random_range(0..24);
    let sent_at = base_time
        .checked_sub(Span::new().days(days_ago).hours(hours_ago))
        .unwrap_or_else(|_| Zoned::now());

    // ~30% read, ~70% unread
    let is_read = rng.random_range(0..100) < 30;

    // Varied notification types
    let notification_type = match index % 5 {
        0 => "appointment_reminder",
        1 => "prescription_ready",
        2 => "lab_result_available",
        3 => "payment_received",
        _ => "system_maintenance",
    };

    let title = match notification_type {
        "appointment_reminder" => {
            format!(
                "Appointment Reminder - {}",
                ["Tomorrow", "Today", "In 2 hours", "Upcoming"][rng.random_range(0..4)]
            )
        }
        "prescription_ready" => {
            format!(
                "Prescription Ready - {}",
                ["Pharmacy A", "Health Plus", "MediCare"][rng.random_range(0..3)]
            )
        }
        "lab_result_available" => {
            format!(
                "Lab Result Available - {}",
                ["Blood Test", "X-Ray", "MRI", "Checkup"][rng.random_range(0..4)]
            )
        }
        "payment_received" => {
            format!(
                "Payment Received - {}",
                ["Consultation Fee", "Procedure", "Follow-up"][rng.random_range(0..3)]
            )
        }
        "system_maintenance" => {
            format!(
                "System Maintenance - {}",
                ["Scheduled", "Completed", "Reminder"][rng.random_range(0..3)]
            )
        }
        _ => "System Alert".to_string(),
    };

    let sub_title = match notification_type {
        "appointment_reminder" => {
            format!(
                "Patient {} has an appointment at {}",
                ["John Doe", "Jane Smith", "Bob Wilson"][rng.random_range(0..3)],
                ["10:00 AM", "2:30 PM", "4:00 PM"][rng.random_range(0..3)]
            )
        }
        "prescription_ready" => {
            format!(
                "Your prescription for {} is ready for pickup",
                ["Amoxicillin", "Ibuprofen", "Vitamin D"][rng.random_range(0..3)]
            )
        }
        "lab_result_available" => "Click to view the detailed results".to_string(),
        "payment_received" => {
            format!("Received ${} for consultation", rng.random_range(50..500))
        }
        "system_maintenance" => "System will be temporarily unavailable".to_string(),
        _ => "Please check your dashboard".to_string(),
    };

    let notification_id = format!("alert_{}", uuid::Uuid::new_v4());

    NotificationDoc::Alert {
        notification_id,
        is_read,
        title,
        sub_title,
        sent_at,
    }
}

/// Generate an Alert notification with a very long title and subTitle (200-500 chars each)
fn generate_long_text_alert_notification(index: usize, base_time: &Zoned) -> NotificationDoc {
    let mut rng = rand::rng();

    let days_ago = rng.random_range(0..30);
    let hours_ago = rng.random_range(0..24);
    let sent_at = base_time
        .checked_sub(Span::new().days(days_ago).hours(hours_ago))
        .unwrap_or_else(|_| Zoned::now());

    let is_read = rng.random_range(0..100) < 30;

    // Long title variants (200-500 characters)
    let title = match index % 5 {
        0 => format!(
            "URGENT: Appointment Reminder — Your patient Mr. Somchai Phothiwat has an upcoming consultation scheduled for tomorrow morning at 09:30 AM in Room 4B, Main Building. Please ensure all relevant medical records and prior test results have been reviewed before the session. This is notification #{index}."
        ),
        1 => format!(
            "Lab Result Alert — Critical blood work results are now available for patient Ms. Nanthipha Kongkaew (HN-{}). The CBC panel and lipid profile have been completed by the central laboratory. Immediate physician review is recommended as several values fall outside the normal reference range.", index * 7 + 1001
        ),
        2 => format!(
            "Prescription Renewal Request — Patient Prawit Ruangsri has submitted a request to renew their ongoing medication regimen including Metformin 500 mg, Amlodipine 5 mg, and Atorvastatin 20 mg. All three prescriptions expire within the next seven days. Please review the request at your earliest convenience. Case #{index}."
        ),
        3 => format!(
            "Payment Confirmation — A consultation fee payment of ฿{} has been successfully processed for the telemedicine session conducted on {} by patient Ratana Suwannaphat. The transaction has been recorded in the billing system and a receipt has been sent to the patient's registered email address.", index * 150 + 350, ["Monday", "Tuesday", "Wednesday", "Thursday"][index % 4]
        ),
        _ => format!(
            "System Maintenance Notice — Scheduled infrastructure maintenance will take place this {} between 02:00 AM and 05:00 AM ICT. During this window, the appointment booking portal, e-prescribing module, and the patient medical record viewer may be temporarily unavailable. Please plan your patient interactions accordingly. Thank you for your cooperation. Reference: MAINT-{:04}.", ["Sunday", "Saturday"][index % 2], index + 100
        ),
    };

    // Long subtitle variants (200-500 characters)
    let sub_title = match index % 5 {
        0 => "Please log in to the doctor portal to review the appointment details, confirm your availability, and prepare any necessary referral documents or imaging orders. If you need to reschedule, contact the scheduling team at least 2 hours in advance to avoid patient inconvenience.".to_string(),
        1 => format!(
            "Flagged values include elevated LDL cholesterol at {} mg/dL (reference: <100), fasting glucose at {} mg/dL (reference: 70–99), and a slightly elevated creatinine at {:.1} mg/dL. The attending physician's notes have been attached to the patient record for your reference.", index * 3 + 142, index * 2 + 110, 1.2 + (index as f64) * 0.05
        ),
        2 => "The patient reports no adverse reactions or side effects from the current medication regimen and requests continuation of the same dosages. Please review the latest HbA1c, blood pressure readings, and liver function tests uploaded earlier this week before approving the renewal request.".to_string(),
        3 => "This payment completes the outstanding balance for the current billing cycle. If you have any concerns regarding the transaction or need to issue a correction, please contact the finance department with the transaction reference number. Automated receipts are available in the billing module.".to_string(),
        _ => "All non-urgent tasks such as report generation, bulk record exports, and appointment confirmations should be completed before the maintenance window begins. Emergency access protocols remain active throughout the maintenance period. For urgent issues, call the 24/7 technical support hotline.".to_string(),
    };

    let notification_id = format!("alert_long_{}", uuid::Uuid::new_v4());

    NotificationDoc::Alert {
        notification_id,
        is_read,
        title,
        sub_title,
        sent_at,
    }
}

/// Generate an Announcement notification with a very long title and subTitle (200-500 chars each)
fn generate_long_text_announcement_notification(
    index: usize,
    base_time: &Zoned,
) -> NotificationDoc {
    let mut rng = rand::rng();

    let days_ago = rng.random_range(0..30);
    let hours_ago = rng.random_range(0..24);
    let sent_at = base_time
        .checked_sub(Span::new().days(days_ago).hours(hours_ago))
        .unwrap_or_else(|_| Zoned::now());

    let is_read = rng.random_range(0..100) < 30;

    let categories = [
        "Pharmacy",
        "Regulation",
        "Marketing",
        "Tech Update",
        "Accounting",
        "Other",
    ];
    let category = categories[index % 6].to_string();

    // Long title variants (200-500 characters)
    let title = match category.as_str() {
        "Pharmacy" => format!(
            "Pharmacy Update — New Formulary Addition: {} medications have been added to the TrueHealth platform formulary effective March 2025. This includes antibiotics, antihypertensives, and diabetes management drugs. Please review the updated prescribing guidelines and dosage recommendations. Document ID: PHARM-{:04}.", index % 20 + 5, index + 1000
        ),
        "Regulation" => format!(
            "Regulatory Compliance Alert — Important changes to medical licensing requirements have been announced by the Thai Medical Council. All practicing physicians must complete the updated {} module by {} to maintain active licensure status. Non-compliance may affect your ability to prescribe controlled substances.", ["Continuing Medical Education", "Ethics Training", "Patient Safety"][index % 3], ["March 31, 2025", "June 30, 2025", "September 30, 2025"][index % 3]
        ),
        "Marketing" => format!(
            "Exclusive Healthcare Provider Promotion — Enjoy up to {}% discount on all telemedicine consultation fees throughout the month of {} 2025! This offer applies to all registered doctors on the TrueHealth platform and includes video, audio, and chat-based consultations. Promo code: DOCTOR{:03}.", index % 40 + 10, ["March", "April", "May"][index % 3], index + 1
        ),
        "Tech Update" => format!(
            "Platform Update v{}.{} Released — We are thrilled to announce the latest version of the TrueHealth Doctor Application, which includes significant performance improvements to the appointment scheduling engine, an upgraded patient history timeline with better filtering, and enhanced push notification reliability across iOS and Android devices.", 2 + index / 10, index % 10
        ),
        "Accounting" => format!(
            "Accounting Notice — Your consultation fee payment statement for {} 2025 is now available. Total payments received: {:.2} THB. A detailed tax invoice (PND. 53) has been generated and can be downloaded from the billing portal. Please review all transactions and report any discrepancies within 7 days.", ["January", "February", "March"][index % 3], (index as f64) * 1500.0 + 5000.0
        ),
        _ => format!(
            "General Announcement — Dear Healthcare Professional, we would like to take this opportunity to thank you for your continued dedication and outstanding contributions to the TrueHealth platform throughout Q{} {}. Your commitment to delivering quality telehealth services has directly improved patient outcomes across all service regions.", index % 4 + 1, 2024 + index / 4
        ),
    };

    // Long subtitle variants (200-500 characters)
    let sub_title = match category.as_str() {
        "Pharmacy" => "The new medications include both brand-name and generic options. Please review the updated drug interaction database as some combinations may require dosage adjustments. A complete list of the newly added medications, including their NDC codes, supplier information, and bulk pricing tiers, has been uploaded to the formulary section of your dashboard.".to_string(),
        "Regulation" => "The updated licensing module is now available in the TrueHealth Learning Management System. You can access the training materials, complete the assessment, and download your completion certificate directly from the platform. Please ensure your profile information is current and matches your official medical council records to avoid processing delays.".to_string(),
        "Marketing" => "To redeem this exclusive offer, navigate to Settings › Billing › Promotions and enter the promo code before the expiry date shown above. The discount will be automatically applied to your next consultation billing cycle. Terms and conditions apply; offer valid for verified TrueHealth partner doctors only.".to_string(),
        "Tech Update" => "To update your application, visit the App Store or Google Play and install the latest version. After updating, please clear the app cache to ensure all new features load correctly. Detailed release notes, including a full list of bug fixes and API changes, are available in the Help Center under 'Release History'.".to_string(),
        "Accounting" => "All payments have been reconciled with your consultation records. The statement includes breakdowns by consultation type, patient demographics, and payment method. For tax purposes, you may download the Withholding Tax Certificate (Form 50) from the documents section. Contact the finance department at accounting@truehealth.co.th for inquiries.".to_string(),
        _ => "We look forward to continuing this journey with you in the coming year and are committed to providing you with the best tools, support, and compensation structure in the industry. Please look out for our upcoming annual performance review invitation, which will be sent to your registered email address within the next 14 days.".to_string(),
    };

    let content_url = format!(
        "https://example.com/announcements/long/{}",
        uuid::Uuid::new_v4()
    );

    let icon_url = match category.as_str() {
        "Pharmacy" => "https://example.com/icons/pharmacy.png".to_string(),
        "Regulation" => "https://example.com/icons/regulation.png".to_string(),
        "Marketing" => "https://example.com/icons/marketing.png".to_string(),
        "Tech Update" => "https://example.com/icons/tech.png".to_string(),
        "Accounting" => "https://example.com/icons/accounting.png".to_string(),
        "Other" => "https://example.com/icons/other.png".to_string(),
        _ => "https://example.com/icons/default.png".to_string(),
    };

    let notification_id = format!("announcement_long_{}", uuid::Uuid::new_v4());

    NotificationDoc::Announcement {
        notification_id,
        is_read,
        title,
        sub_title,
        sent_at,
        content_url,
        icon_url,
        category,
    }
}

/// Generate an Announcement notification with varied content
fn generate_announcement_notification(index: usize, base_time: &Zoned) -> NotificationDoc {
    let mut rng = rand::rng();

    // Generate timestamp spread over last 30 days
    let days_ago = rng.random_range(0..30);
    let hours_ago = rng.random_range(0..24);
    let sent_at = base_time
        .checked_sub(Span::new().days(days_ago).hours(hours_ago))
        .unwrap_or_else(|_| Zoned::now());

    // ~30% read, ~70% unread
    let is_read = rng.random_range(0..100) < 30;

    // Varied categories
    let categories = [
        "Pharmacy",
        "Regulation",
        "Marketing",
        "Tech Update",
        "Accounting",
        "Other",
    ];
    let category = categories[index % 6].to_string();

    let title = match category.as_str() {
        "Pharmacy" => {
            format!(
                "Pharmacy Update - {}",
                [
                    "New Formulary",
                    "Drug Availability",
                    "Prescription Changes",
                    "Medication Recall"
                ][rng.random_range(0..4)]
            )
        }
        "Regulation" => {
            format!(
                "Regulatory Alert - {}",
                [
                    "License Renewal",
                    "Compliance Update",
                    "Policy Change",
                    "New Guidelines"
                ][rng.random_range(0..4)]
            )
        }
        "Marketing" => {
            format!(
                "Marketing Campaign - {}",
                [
                    "New Promotion",
                    "Provider Incentive",
                    "Referral Bonus",
                    "Special Offer"
                ][rng.random_range(0..4)]
            )
        }
        "Tech Update" => {
            format!(
                "Technology Update - {}",
                [
                    "Platform Release",
                    "Feature Enhancement",
                    "System Upgrade",
                    "Performance Boost"
                ][rng.random_range(0..4)]
            )
        }
        "Accounting" => {
            format!(
                "Accounting Notice - {}",
                [
                    "Payment Processed",
                    "Tax Document",
                    "Billing Update",
                    "Invoice Available"
                ][rng.random_range(0..4)]
            )
        }
        _ => {
            format!(
                "General Announcement - {}",
                ["Reminder", "Notice", "Update", "Announcement"][rng.random_range(0..4)]
            )
        }
    };

    let sub_title = match category.as_str() {
        "Pharmacy" => {
            format!(
                "{} - Important pharmacy-related announcement for providers",
                [
                    "Formulary Update",
                    "New Stock Available",
                    "Prescribing Guidelines",
                    "Safety Alert"
                ][rng.random_range(0..4)]
            )
        }
        "Regulation" => "Regulatory compliance announcement - action may be required".to_string(),
        "Marketing" => {
            "Marketing and promotional announcement for healthcare providers".to_string()
        }
        "Tech Update" => "Technology and platform update for improved service delivery".to_string(),
        "Accounting" => "Accounting and billing announcement regarding your payments".to_string(),
        _ => "General announcement for all healthcare providers".to_string(),
    };

    let content_url = format!("https://example.com/announcements/{}", uuid::Uuid::new_v4());

    let icon_url = match category.as_str() {
        "Pharmacy" => "https://example.com/icons/pharmacy.png".to_string(),
        "Regulation" => "https://example.com/icons/regulation.png".to_string(),
        "Marketing" => "https://example.com/icons/marketing.png".to_string(),
        "Tech Update" => "https://example.com/icons/tech.png".to_string(),
        "Accounting" => "https://example.com/icons/accounting.png".to_string(),
        "Other" => "https://example.com/icons/other.png".to_string(),
        _ => "https://example.com/icons/default.png".to_string(),
    };

    let notification_id = format!("announcement_{}", uuid::Uuid::new_v4());

    NotificationDoc::Announcement {
        notification_id,
        is_read,
        title,
        sub_title,
        sent_at,
        content_url,
        icon_url,
        category,
    }
}
