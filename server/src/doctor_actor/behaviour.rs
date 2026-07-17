use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::timeslot::Timeslot;

// pub trait DoctorBehaviour {
//     fn book_timeslot(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
//     fn release_timeslot(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
//     fn enable_instant(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
//     fn enable_schedle(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
//     fn disable_instant(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
//     fn disable_schedle(&self, cmd: todo!()) -> Result<(), anyhow::Error>;
// }
// Getavailable error
#[derive(Debug, Error)]
pub enum GetAvailableTimeslotsError {
    //doctor not found
    #[error("Doctor not found")]
    DoctorNotFound,
    //doctor not available
    #[error("Doctor not available")]
    DoctorNotAvailable,
    //doctor not available in the time range
    #[error("Doctor not available in the time range")]
    DoctorNotAvailableInTimeRange,
}

pub struct GetAvailableTimeslotsParams {
    pub doctor_id: i32,
    //range of time
    pub end_time: i64,
    pub start_time: i64,
}

pub trait GetAvailableTimeslots {
    fn get_available_timeslots(
        &self,
        params: GetAvailableTimeslotsParams,
    ) -> BoxFuture<Result<Vec<Timeslot>, GetAvailableTimeslotsError>>;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReserveTimeslotResult {
    Success,
}

#[derive(Debug, thiserror::Error)]
pub enum ReserveTimeslotError {
    #[error(transparent)]
    ServerError(#[from] anyhow::Error),

    #[error("Timeslot is already reserved")]
    NotAbleToReserve,
}

pub struct ReserveDoctorTimeslotParams {
    pub timeslot_id: i64,
    pub doctor_id: i32,
}

pub trait ReserveDoctorTimeslot {
    fn reserve_doctor_timeslot(
        &self,
        params: ReserveDoctorTimeslotParams,
    ) -> BoxFuture<Result<ReserveTimeslotResult, ReserveTimeslotError>>;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReleaseTimeslotResult {
    Success,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseTimeslotError {
    #[error(transparent)]
    ServerError(#[from] anyhow::Error),
}

pub struct ReleaseDoctorTimeslotParams {
    pub timeslot_id: i64,
    pub doctor_id: i32,
}

pub trait ReleaseDoctorTimeslot {
    fn release_doctor_timeslot(
        &self,
        params: ReleaseDoctorTimeslotParams,
    ) -> BoxFuture<Result<ReleaseTimeslotResult, ReserveTimeslotError>>;
}


