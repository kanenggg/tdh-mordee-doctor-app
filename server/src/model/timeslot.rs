pub struct Timeslot {
    pub id: i64,
    pub protocol_version: i32,
    pub start_at: i64,
    pub end_at: i64,
    pub duration: i64,
}

pub enum ReserveTimeslotStatus {
    Reserved,
    Confirmed,
}
// pub enum BookingType {
//     Instant,
//     Scheduled,
// }
//
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
//     pub reserve_type: ReserveTimeslotType,
// struct ReserveTimeslot {
//     pub booking_type: BookingType,
// }
