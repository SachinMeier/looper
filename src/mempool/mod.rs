extern crate serde;
extern crate serde_json;

use core::time;

// use tokio::task;

use bdk::FeeRate;
use reqwest::Client;
use serde::Deserialize;

// TODO: don't instatiate a new client for every request. Have each function take in a client as param.

const MEMPOOL_BASE_URL: &str = "https://mempool.space/api/v1";
// TODO: make configurable
const REQUEST_TIMEOUT: u64 = 15;

#[derive(Debug)]
pub enum MempoolFeePriority {
    Fastest,
    Blocks3,
    Blocks6,
    Economy,
    Minimum,
}

#[derive(Debug, Deserialize)]
pub struct FeeEstimate {
    #[serde(rename = "fastestFee")]
    pub fastest_fee: u64,
    #[serde(rename = "halfHourFee")]
    pub half_hour_fee: u64,
    #[serde(rename = "hourFee")]
    pub hour_fee: u64,
    #[serde(rename = "economyFee")]
    pub economy_fee: u64,
    #[serde(rename = "minimumFee")]
    pub minimum_fee: u64,
}

fn build_mempool_url(endpoint: &str) -> String {
    format!("{}{}", MEMPOOL_BASE_URL, endpoint)
}

pub async fn get_mempool_fee_estimate() -> Result<FeeEstimate, MempoolError> {
    let client = Client::new();
    let resp = client
        .get(build_mempool_url("/fees/recommended"))
        // TODO: maybe this should be set on the client not the request.
        .timeout(time::Duration::from_secs(REQUEST_TIMEOUT))
        .send()
        .await
        .map_err(|e| {
            MempoolError::new(format!(
                "failed to get mempool fee estimate: {:?}",
                e.to_string()
            ))
        })?;

    let fee_estimate: FeeEstimate = resp.json().await.map_err(|e| {
        MempoolError::new(format!(
            "failed to decode mempool fee estimate response: {:?}",
            e.to_string()
        ))
    })?;

    Ok(fee_estimate)
}

pub async fn get_mempool_fee_rate(priority: MempoolFeePriority) -> Result<FeeRate, MempoolError> {
    let fee_estimate = get_mempool_fee_estimate().await?;
    Ok(get_fee_estimate_by_priority(&fee_estimate, priority))
}

pub fn get_fee_estimate_by_priority(
    fee_estimate: &FeeEstimate,
    priority: MempoolFeePriority,
) -> FeeRate {
    let fee_rate = match priority {
        MempoolFeePriority::Fastest => fee_estimate.fastest_fee as f32,
        MempoolFeePriority::Blocks3 => fee_estimate.half_hour_fee as f32,
        MempoolFeePriority::Blocks6 => fee_estimate.hour_fee as f32,
        MempoolFeePriority::Economy => fee_estimate.economy_fee as f32,
        MempoolFeePriority::Minimum => fee_estimate.minimum_fee as f32,
    };

    FeeRate::from_sat_per_vb(fee_rate)
}

#[derive(Debug)]
pub struct MempoolError {
    pub message: String,
}

impl MempoolError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    // use tokio::test;

    #[tokio::test]
    async fn test_get_mempool_fee_estimate() {
        let fee_estimate = get_mempool_fee_estimate().await.unwrap();

        assert_fee_estimate(&fee_estimate);
    }

    #[tokio::test]
    async fn test_get_mempool_fee_rate() {
        let fee_rate = get_mempool_fee_rate(MempoolFeePriority::Blocks3)
            .await
            .unwrap();

        let zero = FeeRate::from_sat_per_vb(0.0);
        assert!(fee_rate.gt(&zero));
    }

    // #[tokio::test]
    // async fn test_get_mempool_fee_estimate_sync() {
    //     let fee_estimate = get_mempool_fee_estimate_sync(&CLIENT).unwrap();

    //     assert_fee_estimate(&fee_estimate);
    // }

    fn assert_fee_estimate(fee_estimate: &FeeEstimate) {
        assert!(fee_estimate.fastest_fee > 0);
        assert!(
            fee_estimate.half_hour_fee > 0
                && fee_estimate.half_hour_fee <= fee_estimate.fastest_fee
        );
        assert!(fee_estimate.hour_fee > 0 && fee_estimate.hour_fee <= fee_estimate.half_hour_fee);
        assert!(fee_estimate.economy_fee > 0 && fee_estimate.economy_fee <= fee_estimate.hour_fee);
        assert!(
            fee_estimate.minimum_fee > 0 && fee_estimate.minimum_fee <= fee_estimate.economy_fee
        );
    }
}
