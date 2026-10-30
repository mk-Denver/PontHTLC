use std::time::Duration;

use bitcoin::hashes::Hash;
use bitcoin::secp256k1::Secp256k1;
use lightning::sign::KeysManager;
use lightning::types::payment::PaymentSecret;
use lightning_invoice::{Bolt11Invoice, Currency, InvoiceBuilder};

pub fn build_hold_invoice(
    keys_manager: &KeysManager, payment_hash: [u8; 32], payment_secret: PaymentSecret,
    amount_msat: u64,
) -> Result<Bolt11Invoice, String> {
    let secp_ctx = Secp256k1::signing_only();
    let node_secret_key = keys_manager.get_node_secret_key();
    let payment_hash_sha256 = bitcoin::hashes::sha256::Hash::from_byte_array(payment_hash);
    InvoiceBuilder::new(Currency::Regtest)
        .amount_milli_satoshis(amount_msat)
        .description("PontHTLC hold invoice".to_string())
        .payment_hash(payment_hash_sha256)
        .payment_secret(payment_secret)
        .current_timestamp()
        .min_final_cltv_expiry_delta(40)
        .expiry_time(Duration::from_secs(3600))
        .build_signed(|message| secp_ctx.sign_ecdsa_recoverable(message, &node_secret_key))
        .map_err(|err| format!("invoice build_signed failed: {:?}", err))
}

pub fn parse_bolt11(invoice: &str) -> Result<Bolt11Invoice, String> {
    invoice.parse().map_err(|err| format!("invalid bolt11: {err:?}"))
}

pub fn payment_hash_bytes(invoice: &Bolt11Invoice) -> [u8; 32] {
    invoice.payment_hash().to_byte_array()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoice_round_trips_the_payment_hash() {
        let preimage = [0xeeu8; 32];
        let payment_hash = bitcoin::hashes::sha256::Hash::hash(&preimage).to_byte_array();
        let keys = KeysManager::new(&[7u8; 32], 1, 0, true);
        let invoice = build_hold_invoice(
            &keys,
            payment_hash,
            PaymentSecret([9u8; 32]),
            50_000_000,
        )
        .unwrap();
        let text = invoice.to_string();
        assert!(!text.contains(&hex::encode(preimage)));
        let parsed = parse_bolt11(&text).unwrap();
        assert_eq!(payment_hash_bytes(&parsed), payment_hash);
        assert_eq!(parsed.description().to_string(), "PontHTLC hold invoice");
    }
}
