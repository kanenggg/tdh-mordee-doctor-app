//! Pure helper functions used by the BFF handler to compose its
//! response from the three upstream payloads.

use jiff::civil::Date;
use jiff::Timestamp;

use crate::model::appointment_status::AppointmentCardStatus;
use crate::module::appointment::external::{ConsultationDetail, MorDeeUserProfile, PaymentDetail};
use crate::module::appointment::model::{
    AppointmentTime, Coupon, Patient, Payment, Prescreen, SuccessBody,
};

/// Strip the leading two-character `"BK"` booking prefix if present;
/// otherwise pass the input through unchanged.
pub fn derive_appointment_no(booking_id: &str) -> String {
    if let Some(rest) = booking_id.strip_prefix("BK") {
        rest.to_string()
    } else {
        booking_id.to_string()
    }
}

/// Compute integer age in completed years.
///
/// Returns `today.year - dob.year`, minus 1 if today's `(month, day)` is
/// before the dob's `(month, day)`. Strict month/day comparison handles
/// leap-day birthdays correctly: a Feb-29 dob "reaches" age N+1 on
/// March 1 of non-leap years.
///
/// If `dob` is in the future relative to `today`, the result is negative.
/// Callers that need a non-negative display value (e.g. the BFF response)
/// must clamp at their layer.
pub fn compute_age(dob: Date, today: Date) -> i32 {
    let mut age = today.year() as i32 - dob.year() as i32;
    let today_md = (today.month(), today.day());
    let dob_md = (dob.month(), dob.day());
    if today_md < dob_md {
        age -= 1;
    }
    age
}

/// Slugify a campaign name to fit a `{couponKey}` URL placeholder.
///
/// 1. Lowercase
/// 2. Replace each run of non-`[a-z0-9]` characters with a single `-`
/// 3. Trim leading/trailing `-`
///
/// Returns an empty string if no `[a-z0-9]` characters remain.
pub fn slugify_campaign(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true; // start "true" so leading separators don't emit
    for ch in s.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    // Trim trailing dash if any.
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// Substitute a single `{key}` placeholder in `template` with a
/// lowercased `key_value`. Caller is responsible for having validated
/// the template at startup via [`validate_url_template`].
///
/// The `placeholder` must be non-empty; passing an empty placeholder
/// would inject the key value between every character of the template.
pub fn build_url_from_template(template: &str, placeholder: &str, key_value: &str) -> String {
    template.replace(placeholder, &key_value.to_ascii_lowercase())
}

/// Validate that `template` contains the `placeholder` literal exactly
/// once. Returns the template back unchanged on success, or a
/// human-readable error message on failure. Used at startup so an
/// invalid template fails fast instead of producing broken URLs at
/// request time.
pub fn validate_url_template<'a>(template: &'a str, placeholder: &str) -> Result<&'a str, String> {
    let count = template.matches(placeholder).count();
    match count {
        1 => Ok(template),
        0 => Err(format!(
            "URL template {:?} is missing the required placeholder {:?}",
            template, placeholder
        )),
        n => Err(format!(
            "URL template {:?} contains the placeholder {:?} {} times; expected exactly 1",
            template, placeholder, n
        )),
    }
}

use tracing::warn;

use crate::module::appointment::external::{PaymentChannel, SelectedChannelResult};

/// Lean view of the payer pulled out of the upstream
/// `selectedChannelResult` payload. Used by the handler to populate
/// `Payment.payer_name`, `Payment.has_insurance`, and to feed
/// `build_insurance_url`.
pub struct PayerInfo {
    pub payer_name: String,
    pub has_insurance: bool,
    /// The privilege id the URL builder slots into `{privilegeId}`.
    /// `None` when `has_insurance` is false, OR when it's true but the
    /// upstream insurance channel carried no `privilegeId` (e.g. legacy
    /// Insurance v1/v2, or a v3 payload with the field absent).
    pub privilege_id: Option<i64>,
}

pub fn extract_payer(scr: Option<&SelectedChannelResult>) -> PayerInfo {
    match scr {
        None => PayerInfo {
            payer_name: "Free".to_string(),
            has_insurance: false,
            privilege_id: None,
        },
        Some(SelectedChannelResult::SelfPay { .. }) => PayerInfo {
            payer_name: "Self pay".to_string(),
            has_insurance: false,
            privilege_id: None,
        },
        Some(SelectedChannelResult::Coverage { channel })
        | Some(SelectedChannelResult::CoverageAndSelfPay {
            coverage_channel: channel,
            ..
        }) => from_coverage_channel(channel),
    }
}

fn from_coverage_channel(channel: &PaymentChannel) -> PayerInfo {
    match channel {
        PaymentChannel::Insurance {
            insurer_code,
            insurance_name_i18n,
        }
        | PaymentChannel::InsuranceV2 {
            insurer_code,
            insurance_name_i18n,
        } => {
            let i18n_en = insurance_name_i18n.as_ref().and_then(|m| m.en.clone());
            let payer_name = first_non_empty([i18n_en, insurer_code.clone()])
                .unwrap_or_else(|| "Insurance".to_string());
            // Legacy v1/v2 carry no privilegeId, so they get no condition URL.
            PayerInfo {
                payer_name,
                has_insurance: true,
                privilege_id: None,
            }
        }
        PaymentChannel::InsuranceV3 {
            provider_name,
            provider_abbreviation,
            insurance_name_i18n,
            privilege_id,
        } => {
            let i18n_en = insurance_name_i18n.as_ref().and_then(|m| m.en.clone());
            let payer_name = first_non_empty([
                provider_name.clone(),
                i18n_en,
                provider_abbreviation.clone(),
            ])
            .unwrap_or_else(|| "Insurance".to_string());
            PayerInfo {
                payer_name,
                has_insurance: true,
                privilege_id: *privilege_id,
            }
        }
        PaymentChannel::EmployeeBenefit { company_name }
        | PaymentChannel::EmployeeBenefitV2 { company_name } => PayerInfo {
            payer_name: first_non_empty([company_name.clone()])
                .unwrap_or_else(|| "Employee Benefit".to_string()),
            has_insurance: false,
            privilege_id: None,
        },
        PaymentChannel::CampaignLocation => PayerInfo {
            payer_name: "Campaign".to_string(),
            has_insurance: false,
            privilege_id: None,
        },
        // Self-pay channels appearing under Coverage (shouldn't happen
        // in practice but we're defensive) and the Unknown catch-all
        // both fall through to "Self pay" — the mobile UI has nothing
        // useful to render for an unclassified coverage channel, so a
        // generic self-pay label is better than an orphan insurance
        // badge with no T&C link. The warn surfaces the anomaly so
        // on-call can chase it before it becomes a regression.
        other => {
            let kind = match other {
                PaymentChannel::Card => "Card",
                PaymentChannel::PromptPay => "PromptPay",
                PaymentChannel::TrueMoney => "TrueMoney",
                PaymentChannel::CardSchedule => "CardSchedule",
                PaymentChannel::Unknown => "Unknown",
                _ => unreachable!("structured variants handled above"),
            };
            warn!(
                channel_kind = kind,
                "unclassified coverage channel; falling back to Self pay"
            );
            PayerInfo {
                payer_name: "Self pay".to_string(),
                has_insurance: false,
                privilege_id: None,
            }
        }
    }
}

fn first_non_empty<I: IntoIterator<Item = Option<String>>>(opts: I) -> Option<String> {
    opts.into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
}

/// Borrowed view over the two URL templates the handler reads from
/// config. Passed by value through the mapper so the helpers stay
/// dependency-injected and trivially unit-testable.
#[derive(Clone, Copy)]
pub struct Templates<'a> {
    pub insurance: &'a str,
    pub coupon: &'a str,
}

const INSURANCE_PLACEHOLDER: &str = "{privilegeId}";
const COUPON_PLACEHOLDER: &str = "{couponKey}";

/// Compose the BFF response body from the three upstream payloads and
/// the URL templates. `today` is injected so the function is
/// deterministic and the age computation can be unit-tested.
pub fn compose(
    consultation: ConsultationDetail,
    profile: MorDeeUserProfile,
    payment: Option<PaymentDetail>,
    templates: Templates<'_>,
    today: Date,
) -> SuccessBody {
    let appointment_no = derive_appointment_no(&consultation.booking_id);
    let appointment_date = utc_date_string(consultation.appointment_time.start_time);

    let dob_parsed = profile
        .date_of_birth
        .as_ref()
        .and_then(|s| s.parse::<Date>().ok());
    let age = dob_parsed.map(|d| compute_age(d, today));

    let full_name = build_full_name(profile.first_name.as_deref(), profile.last_name.as_deref());

    let patient = Patient {
        account_id: consultation.patient.account_id,
        profile_id: consultation.patient.profile_id,
        full_name,
        date_of_birth: profile.date_of_birth,
        age,
        gender: profile.gender,
    };

    let (payment_obj, coupon_obj) = match payment {
        None => (None, None),
        Some(p) => {
            let payer = extract_payer(p.selected_channel_result.as_ref());
            let insurance_condition_url = if payer.has_insurance {
                payer.privilege_id.map(|id| {
                    build_url_from_template(
                        templates.insurance,
                        INSURANCE_PLACEHOLDER,
                        &id.to_string(),
                    )
                })
            } else {
                None
            };
            let coupon = extract_coupon(p.coupon_protocol.as_ref(), templates.coupon);
            let payment = Payment {
                payment_tx_id: p.payment_transaction_id,
                payment_tx_ref_id: p.payment_transaction_ref_id,
                payer_name: payer.payer_name,
                has_insurance: payer.has_insurance,
                insurance_condition_url,
                amount: p.amount,
            };
            (Some(payment), coupon)
        }
    };

    SuccessBody {
        booking_id: consultation.booking_id,
        appointment_no,
        appointment_time: AppointmentTime {
            start_time: consultation.appointment_time.start_time,
            end_time: consultation.appointment_time.end_time,
        },
        appointment_date,
        status: AppointmentCardStatus::from(consultation.status),
        booking_type: consultation.booking_type,
        consultation_channel: consultation.consultation_channel,
        patient,
        payment: payment_obj,
        coupon: coupon_obj,
        prescreen: Prescreen {
            symptom: consultation.prescreen.symptom,
            duration: consultation.prescreen.duration,
            duration_unit: consultation.prescreen.duration_unit,
            attachments: consultation.prescreen.attachments,
            allergies: consultation.prescreen.allergies,
        },
    }
}

fn build_full_name(first: Option<&str>, last: Option<&str>) -> Option<String> {
    let f = first.map(str::trim).filter(|s| !s.is_empty());
    let l = last.map(str::trim).filter(|s| !s.is_empty());
    match (f, l) {
        (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
        (Some(f), None) => Some(f.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
    }
}

fn utc_date_string(epoch_seconds: i64) -> String {
    Timestamp::from_second(epoch_seconds)
        .map(|ts| {
            let d = ts.to_zoned(jiff::tz::TimeZone::UTC).date();
            format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
        })
        .unwrap_or_default()
}

fn extract_coupon(
    coupon_protocol: Option<&serde_json::Value>,
    coupon_template: &str,
) -> Option<Coupon> {
    let proto = coupon_protocol?;
    let raw_name = proto.get("campaignName").and_then(|v| v.as_str())?;
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        warn!(
            upstream_type = ?proto.get("__type"),
            "coupon campaignName missing or empty",
        );
        return None;
    }
    let slug = slugify_campaign(trimmed);
    let condition_url = if slug.is_empty() {
        warn!(
            campaign_name = trimmed,
            "coupon campaignName slugifies to empty; conditionUrl null",
        );
        None
    } else {
        Some(build_url_from_template(
            coupon_template,
            COUPON_PLACEHOLDER,
            &slug,
        ))
    };
    Some(Coupon {
        campaign_name: trimmed.to_string(),
        condition_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::appointment::external::consultation_client::{
        ConsultationAppointmentTime, ConsultationDetail, ConsultationIdentity,
        ConsultationPrescreen,
    };
    use crate::module::appointment::external::payment_client::{
        I18nMap, PaymentChannel, PaymentDetail, SelectedChannelResult,
    };
    use serde_json::json;

    #[test]
    fn appointment_no_strips_bk_prefix() {
        assert_eq!(derive_appointment_no("BK20220227810949"), "20220227810949");
    }

    #[test]
    fn appointment_no_passes_through_when_no_bk_prefix() {
        assert_eq!(derive_appointment_no("XX20220227"), "XX20220227");
        assert_eq!(derive_appointment_no("20220227"), "20220227");
    }

    #[test]
    fn appointment_no_handles_short_strings() {
        assert_eq!(derive_appointment_no("BK"), "");
        assert_eq!(derive_appointment_no("B"), "B");
        assert_eq!(derive_appointment_no(""), "");
    }

    use jiff::civil::date;

    #[test]
    fn age_basic() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 4, 1);
        assert_eq!(compute_age(dob, today), 45);
    }

    #[test]
    fn age_birthday_not_yet_reached() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 3, 21);
        assert_eq!(compute_age(dob, today), 44);
    }

    #[test]
    fn age_birthday_today() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 3, 22);
        assert_eq!(compute_age(dob, today), 45);
    }

    #[test]
    fn age_leap_year_dob() {
        // Born on a leap day. On a non-leap year, the birthday is treated as
        // March 1 (i.e. one day after Feb 28). Compute strictly by month/day.
        let dob = date(1996, 2, 29);
        // Feb 28, 2026 — birthday not yet reached.
        assert_eq!(compute_age(dob, date(2026, 2, 28)), 29);
        // Mar 1, 2026 — birthday reached.
        assert_eq!(compute_age(dob, date(2026, 3, 1)), 30);
        // Feb 29, 2024 (leap year) — birthday reached.
        assert_eq!(compute_age(dob, date(2024, 2, 29)), 28);
    }

    #[test]
    fn slug_basic() {
        assert_eq!(slugify_campaign("New Year Sale 2026"), "new-year-sale-2026");
    }

    #[test]
    fn slug_punctuation_em_dash_apostrophe() {
        assert_eq!(
            slugify_campaign("50% OFF — Doctor's Day!"),
            "50-off-doctor-s-day"
        );
    }

    #[test]
    fn slug_underscores_and_padding() {
        assert_eq!(slugify_campaign("  TDH_Promo  "), "tdh-promo");
    }

    #[test]
    fn slug_only_punctuation_returns_empty() {
        assert_eq!(slugify_campaign("!!!"), "");
        assert_eq!(slugify_campaign("---"), "");
    }

    #[test]
    fn slug_thai_characters_strip_to_empty() {
        assert_eq!(slugify_campaign("โปรปีใหม่"), "");
    }

    #[test]
    fn slug_collapses_multiple_separators() {
        assert_eq!(slugify_campaign("a   b___c"), "a-b-c");
    }

    #[test]
    fn slug_already_hyphenated_input_passes_through() {
        // Single hyphens between alnum runs hit the "non-alnum + prev was alnum"
        // branch and emit a single dash — same as a space would.
        assert_eq!(slugify_campaign("already-fine"), "already-fine");
        assert_eq!(slugify_campaign("a-b"), "a-b");
    }

    #[test]
    fn url_template_substitutes_placeholder() {
        let url = build_url_from_template(
            "https://static.tdh.com/insurance/{insurerKey}.html",
            "{insurerKey}",
            "aia",
        );
        assert_eq!(url, "https://static.tdh.com/insurance/aia.html");
    }

    #[test]
    fn url_template_lowercases_key() {
        let url = build_url_from_template(
            "https://static.tdh.com/insurance/{insurerKey}.html",
            "{insurerKey}",
            "AIA",
        );
        assert_eq!(url, "https://static.tdh.com/insurance/aia.html");
    }

    #[test]
    fn url_template_validation_accepts_exact_one_placeholder() {
        assert!(validate_url_template("https://x/{insurerKey}.html", "{insurerKey}").is_ok());
    }

    #[test]
    fn url_template_validation_rejects_missing_placeholder() {
        assert!(validate_url_template("https://x/foo.html", "{insurerKey}").is_err());
    }

    #[test]
    fn url_template_validation_rejects_duplicate_placeholder() {
        assert!(
            validate_url_template("https://x/{insurerKey}/{insurerKey}.html", "{insurerKey}")
                .is_err()
        );
    }

    #[test]
    fn payer_self_pay_promptpay() {
        let scr = Some(SelectedChannelResult::SelfPay {
            channel: PaymentChannel::PromptPay,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
        assert!(p.privilege_id.is_none());
    }

    #[test]
    fn payer_self_pay_card() {
        let scr = Some(SelectedChannelResult::SelfPay {
            channel: PaymentChannel::Card,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_insurance_v1_uses_insurer_code() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA");
        assert!(p.has_insurance);
        // Legacy v1 carries no privilegeId, so no condition URL.
        assert!(p.privilege_id.is_none());
    }

    #[test]
    fn payer_insurance_v1_prefers_i18n_over_code() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: Some(I18nMap {
                    en: Some("AIA Health".to_string()),
                }),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA Health");
        assert!(p.has_insurance);
        assert!(p.privilege_id.is_none());
    }

    #[test]
    fn payer_insurance_v3_uses_provider_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: Some("ACME Insurance".to_string()),
                provider_abbreviation: Some("ACME".to_string()),
                insurance_name_i18n: None,
                privilege_id: Some(1001),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "ACME Insurance");
        assert!(p.has_insurance);
        assert_eq!(p.privilege_id, Some(1001));
    }

    #[test]
    fn payer_insurance_v3_falls_back_to_abbreviation_when_no_provider_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: Some("ACME".to_string()),
                insurance_name_i18n: None,
                privilege_id: Some(1001),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "ACME");
        assert!(p.has_insurance);
        assert_eq!(p.privilege_id, Some(1001));
    }

    #[test]
    fn payer_insurance_v3_no_privilege_id_returns_none() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: None,
                insurance_name_i18n: None,
                privilege_id: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Insurance");
        assert!(p.has_insurance);
        assert!(p.privilege_id.is_none());
    }

    #[test]
    fn payer_employee_benefit_not_insurance() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::EmployeeBenefit {
                company_name: Some("Acme Corp".to_string()),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Acme Corp");
        assert!(!p.has_insurance);
        assert!(p.privilege_id.is_none());
    }

    #[test]
    fn payer_employee_benefit_no_company_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::EmployeeBenefit { company_name: None },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Employee Benefit");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_campaign_location() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::CampaignLocation,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Campaign");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_split_uses_coverage_channel() {
        let scr = Some(SelectedChannelResult::CoverageAndSelfPay {
            coverage_channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: None,
            },
            self_pay_channel: PaymentChannel::PromptPay,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA");
        assert!(p.has_insurance);
    }

    #[test]
    fn payer_null_selected_channel_is_free() {
        let p = extract_payer(None);
        assert_eq!(p.payer_name, "Free");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_unknown_channel_falls_back_to_self_pay() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Unknown,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
    }

    fn fixture_consultation() -> ConsultationDetail {
        ConsultationDetail {
            booking_id: "BK20220227810949".to_string(),
            appointment_time: ConsultationAppointmentTime {
                start_time: 1645940400, // 2022-02-27 03:00 UTC
                end_time: 1645941300,
            },
            status: "BOOKED".to_string(),
            booking_type: "Schedule".to_string(),
            consultation_channel: "video".to_string(),
            patient: ConsultationIdentity {
                account_id: 124236,
                profile_id: 200,
            },
            doctor: ConsultationIdentity {
                account_id: 300,
                profile_id: 400,
            },
            prescreen: ConsultationPrescreen {
                symptom: "headache".to_string(),
                duration: 7,
                duration_unit: "day".to_string(),
                attachments: vec!["att-001".to_string()],
                allergies: vec!["Amoxicillin".to_string()],
            },
            payment_tx_id: 1042,
            payment_tx_ref_id: "PT-2026-001".to_string(),
        }
    }

    fn fixture_iam_profile() -> MorDeeUserProfile {
        MorDeeUserProfile {
            first_name: Some("Mrs.Bunyang".to_string()),
            last_name: Some("Lopez".to_string()),
            gender: Some("Female".to_string()),
            date_of_birth: Some("1957-03-22".to_string()),
        }
    }

    fn fixture_payment_insurance_v1() -> PaymentDetail {
        PaymentDetail {
            payment_transaction_id: 1042,
            payment_transaction_ref_id: "PT-2026-001".to_string(),
            amount: serde_json::Number::from_f64(1500.0).unwrap(),
            selected_channel_result: Some(SelectedChannelResult::Coverage {
                channel: PaymentChannel::Insurance {
                    insurer_code: Some("AIA".to_string()),
                    insurance_name_i18n: None,
                },
            }),
            coupon_protocol: None,
        }
    }

    fn fixture_templates() -> Templates<'static> {
        Templates {
            insurance: "https://static.tdh.com/insurance/{privilegeId}.html",
            coupon: "https://static.tdh.com/coupon/{couponKey}.html",
        }
    }

    fn today_2002() -> Date {
        date(2002, 4, 1)
    }

    #[test]
    fn compose_happy_insurance_v1() {
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert_eq!(body.booking_id, "BK20220227810949");
        assert_eq!(body.appointment_no, "20220227810949");
        assert_eq!(body.appointment_date, "2022-02-27");
        assert_eq!(body.status, AppointmentCardStatus::UpComing);
        assert_eq!(body.consultation_channel, "video");

        assert_eq!(body.patient.account_id, 124236);
        assert_eq!(body.patient.full_name.as_deref(), Some("Mrs.Bunyang Lopez"));
        assert_eq!(body.patient.age, Some(45));
        assert_eq!(body.patient.gender.as_deref(), Some("Female"));

        let payment = body.payment.expect("payment populated");
        assert_eq!(payment.payment_tx_id, 1042);
        assert_eq!(payment.payer_name, "AIA");
        assert!(payment.has_insurance);
        // Legacy v1 has no privilegeId, so no condition URL.
        assert!(payment.insurance_condition_url.is_none());

        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_insurance_v3_builds_url_from_privilege_id() {
        let mut payment = fixture_payment_insurance_v1();
        payment.selected_channel_result = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: Some("ACME Insurance".to_string()),
                provider_abbreviation: Some("ACME".to_string()),
                insurance_name_i18n: None,
                privilege_id: Some(1001),
            },
        });
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let payment = body.payment.unwrap();
        assert!(payment.has_insurance);
        assert_eq!(
            payment.insurance_condition_url.as_deref(),
            Some("https://static.tdh.com/insurance/1001.html")
        );
    }

    #[test]
    fn compose_payment_none_payment_field_null() {
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            None, // payment-svc returned NotFound
            fixture_templates(),
            today_2002(),
        );
        assert!(body.payment.is_none());
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_full_name_null_when_both_missing() {
        let mut profile = fixture_iam_profile();
        profile.first_name = None;
        profile.last_name = None;
        let body = compose(
            fixture_consultation(),
            profile,
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.patient.full_name.is_none());
    }

    #[test]
    fn compose_age_null_when_dob_missing() {
        let mut profile = fixture_iam_profile();
        profile.date_of_birth = None;
        let body = compose(
            fixture_consultation(),
            profile,
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.patient.date_of_birth.is_none());
        assert!(body.patient.age.is_none());
    }

    #[test]
    fn compose_insurance_v3_no_privilege_id_url_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.selected_channel_result = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: None,
                insurance_name_i18n: None,
                privilege_id: None,
            },
        });
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let payment = body.payment.unwrap();
        assert_eq!(payment.payer_name, "Insurance");
        assert!(payment.has_insurance);
        assert!(payment.insurance_condition_url.is_none());
    }

    #[test]
    fn compose_coupon_happy_path() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "New Year Sale 2026",
            "coupon": "XMAS2026",
            "couponCampaignId": 99
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let coupon = body.coupon.unwrap();
        assert_eq!(coupon.campaign_name, "New Year Sale 2026");
        assert_eq!(
            coupon.condition_url.as_deref(),
            Some("https://static.tdh.com/coupon/new-year-sale-2026.html")
        );
    }

    #[test]
    fn compose_coupon_missing_campaign_name_yields_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "coupon": "XMAS2026"
            // no campaignName
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_coupon_empty_campaign_name_yields_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "   "
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_coupon_slug_to_empty_keeps_name_drops_url() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "!!!"
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let coupon = body.coupon.unwrap();
        assert_eq!(coupon.campaign_name, "!!!");
        assert!(coupon.condition_url.is_none());
    }

    #[test]
    fn full_name_first_only() {
        let body = compose(
            {
                let mut c = fixture_consultation();
                c.patient.account_id = 1;
                c
            },
            MorDeeUserProfile {
                first_name: Some("Solo".to_string()),
                last_name: None,
                gender: None,
                date_of_birth: None,
            },
            None,
            fixture_templates(),
            today_2002(),
        );
        assert_eq!(body.patient.full_name.as_deref(), Some("Solo"));
    }
}
