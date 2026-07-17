pub mod consultation_client;
pub mod ehr_client;
pub mod iam_client;
pub mod payment_client;
// pub mod qolphin_client;

pub use consultation_client::{
    ConsultationClient, ConsultationClientTrait, ConsultationDetail, ConsultationLookup,
};
pub use iam_client::{IamClient, IamClientTrait, IamLookup, MorDeeUserProfile};
pub use payment_client::{
    PaymentChannel, PaymentClient, PaymentClientTrait, PaymentDetail, PaymentLookup,
    SelectedChannelResult,
};
// pub use qolphin_client::{QolphinClient, QolphinClientTrait};
