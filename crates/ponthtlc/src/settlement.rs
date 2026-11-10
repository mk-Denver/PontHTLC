use crate::tenancy::EscrowRecord;

pub struct PayoutPlan {
    pub escrow_id: String,
    pub address: String,
    pub amount_msat: u64,
    pub side: &'static str,
}

pub fn plan(record: &EscrowRecord, kind: &str) -> Option<PayoutPlan> {
    if record.payout == "paid" || record.amount_msat == 0 {
        return None;
    }
    let (side, address) = match kind {
        "PaymentSent" => ("buyer", record.buyer_lightning_address.as_str()),
        "PaymentFailed" => ("seller", record.seller_lightning_address.as_str()),
        _ => return None,
    };
    if address.is_empty() || (!record.payout.is_empty() && record.payout != side) {
        return None;
    }
    Some(PayoutPlan {
        escrow_id: record.id.clone(),
        address: address.to_string(),
        amount_msat: record.amount_msat,
        side,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> EscrowRecord {
        EscrowRecord {
            id: "escrow-1".to_string(),
            owner: "buyer-key".to_string(),
            amount_msat: 50_000_000,
            buyer_lightning_address: "buyer@example.com".to_string(),
            seller_owner: "seller-key".to_string(),
            seller_lightning_address: "seller@example.com".to_string(),
            ..EscrowRecord::default()
        }
    }

    #[test]
    fn sent_pays_buyer_and_failed_pays_seller() {
        let open = record();
        let sent = plan(&open, "PaymentSent").unwrap();
        assert_eq!(sent.side, "buyer");
        assert_eq!(sent.address, "buyer@example.com");
        assert_eq!(sent.amount_msat, 50_000_000);
        let failed = plan(&open, "PaymentFailed").unwrap();
        assert_eq!(failed.side, "seller");
        assert_eq!(failed.address, "seller@example.com");
        assert!(plan(&open, "PaymentClaimable").is_none());
    }

    #[test]
    fn paid_or_missing_address_is_not_planned() {
        let mut done = record();
        done.payout = "paid".to_string();
        assert!(plan(&done, "PaymentSent").is_none());
        let mut bare = record();
        bare.buyer_lightning_address.clear();
        assert!(plan(&bare, "PaymentSent").is_none());
    }
}
