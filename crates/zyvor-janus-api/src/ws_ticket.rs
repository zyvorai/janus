// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::auth::constant_time_eq;

type HmacSha256 = Hmac<Sha256>;

/// Short-lived on purpose: the ticket only needs to survive the round trip
/// from `POST /api/runs/:id/ws-ticket` to the browser opening the WS
/// connection, not the lifetime of the run itself.
const TICKET_TTL_SECONDS: i64 = 45;

/// Mints a `{run_id}.{expires_at}.{hex_hmac}` ticket, signed with
/// `ZYVOR_JANUS_WS_TICKET_SECRET` (deliberately a distinct secret from
/// `ZYVOR_JANUS_API_KEY` -- see auth.rs and the plan's WS auth design).
pub fn mint_ticket(secret: &str, run_id: Uuid) -> String {
    let expires_at = Utc::now().timestamp() + TICKET_TTL_SECONDS;
    let payload = format!("{run_id}.{expires_at}");
    let sig = sign(secret, &payload);
    format!("{payload}.{sig}")
}

/// Verifies a ticket was minted for exactly this `run_id`, hasn't expired,
/// and carries a valid signature.
pub fn verify_ticket(secret: &str, run_id: Uuid, ticket: &str) -> bool {
    let mut parts = ticket.splitn(3, '.');
    let (Some(id_part), Some(exp_part), Some(sig_part)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if id_part != run_id.to_string() {
        return false;
    }
    let Ok(expires_at) = exp_part.parse::<i64>() else {
        return false;
    };
    if Utc::now().timestamp() > expires_at {
        return false;
    }
    let payload = format!("{id_part}.{exp_part}");
    let expected = sign(secret, &payload);
    constant_time_eq(expected.as_bytes(), sig_part.as_bytes())
}

fn sign(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_valid_ticket() {
        let run_id = Uuid::new_v4();
        let ticket = mint_ticket("secret", run_id);
        assert!(verify_ticket("secret", run_id, &ticket));
    }

    #[test]
    fn rejects_wrong_run_id() {
        let run_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let ticket = mint_ticket("secret", run_id);
        assert!(!verify_ticket("secret", other_id, &ticket));
    }

    #[test]
    fn rejects_tampered_signature() {
        let run_id = Uuid::new_v4();
        let mut ticket = mint_ticket("secret", run_id);
        ticket.push('0');
        assert!(!verify_ticket("secret", run_id, &ticket));
    }

    #[test]
    fn rejects_wrong_secret() {
        let run_id = Uuid::new_v4();
        let ticket = mint_ticket("secret", run_id);
        assert!(!verify_ticket("different-secret", run_id, &ticket));
    }

    #[test]
    fn rejects_expired_ticket() {
        let run_id = Uuid::new_v4();
        let expires_at = Utc::now().timestamp() - 1;
        let payload = format!("{run_id}.{expires_at}");
        let sig = sign("secret", &payload);
        let ticket = format!("{payload}.{sig}");
        assert!(!verify_ticket("secret", run_id, &ticket));
    }
}
